//! Basic syntax highlighting functionality.
//!
//! This module uses librustc_ast's lexer to provide token-based highlighting for
//! the HTML documentation generated by rustdoc.
//!
//! Use the `render_with_highlighting` to highlight some rust code.

use crate::html::escape::Escape;

use std::fmt::{Display, Write};
use std::iter::Peekable;

use rustc_lexer::{LiteralKind, TokenKind};
use rustc_span::symbol::Ident;
use rustc_span::with_default_session_globals;

/// Highlights `src`, returning the HTML output.
pub fn render_with_highlighting(
    src: String,
    class: Option<&str>,
    playground_button: Option<&str>,
    tooltip: Option<(&str, &str)>,
) -> String {
    debug!("highlighting: ================\n{}\n==============", src);
    let mut out = String::with_capacity(src.len());
    if let Some((tooltip, class)) = tooltip {
        write!(
            out,
            "<div class='information'><div class='tooltip {}'>ⓘ<span \
                  class='tooltiptext'>{}</span></div></div>",
            class, tooltip
        )
        .unwrap();
    }

    write_header(&mut out, class);
    write_code(&mut out, &src);
    write_footer(&mut out, playground_button);

    out
}

fn write_header(out: &mut String, class: Option<&str>) {
    write!(out, "<div class=\"example-wrap\"><pre class=\"rust {}\">\n", class.unwrap_or_default())
        .unwrap()
}

fn write_code(out: &mut String, src: &str) {
    Classifier::new(src).highlight(&mut |highlight| {
        match highlight {
            Highlight::Token { text, class } => string(out, Escape(text), class),
            Highlight::EnterSpan { class } => enter_span(out, class),
            Highlight::ExitSpan => exit_span(out),
        };
    });
}

fn write_footer(out: &mut String, playground_button: Option<&str>) {
    write!(out, "</pre>{}</div>\n", playground_button.unwrap_or_default()).unwrap()
}

/// How a span of text is classified. Mostly corresponds to token kinds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Class {
    None,
    Comment,
    DocComment,
    Attribute,
    KeyWord,
    // Keywords that do pointer/reference stuff.
    RefKeyWord,
    Self_,
    Op,
    Macro,
    MacroNonTerminal,
    String,
    Number,
    Bool,
    Ident,
    Lifetime,
    PreludeTy,
    PreludeVal,
    QuestionMark,
}

impl Class {
    /// Returns the css class expected by rustdoc for each `Class`.
    fn as_html(self) -> &'static str {
        match self {
            Class::None => "",
            Class::Comment => "comment",
            Class::DocComment => "doccomment",
            Class::Attribute => "attribute",
            Class::KeyWord => "kw",
            Class::RefKeyWord => "kw-2",
            Class::Self_ => "self",
            Class::Op => "op",
            Class::Macro => "macro",
            Class::MacroNonTerminal => "macro-nonterminal",
            Class::String => "string",
            Class::Number => "number",
            Class::Bool => "bool-val",
            Class::Ident => "ident",
            Class::Lifetime => "lifetime",
            Class::PreludeTy => "prelude-ty",
            Class::PreludeVal => "prelude-val",
            Class::QuestionMark => "question-mark",
        }
    }
}

enum Highlight<'a> {
    Token { text: &'a str, class: Class },
    EnterSpan { class: Class },
    ExitSpan,
}

struct TokenIter<'a> {
    src: &'a str,
}

impl Iterator for TokenIter<'a> {
    type Item = (TokenKind, &'a str);
    fn next(&mut self) -> Option<(TokenKind, &'a str)> {
        if self.src.is_empty() {
            return None;
        }
        let token = rustc_lexer::first_token(self.src);
        let (text, rest) = self.src.split_at(token.len);
        self.src = rest;
        Some((token.kind, text))
    }
}

/// Processes program tokens, classifying strings of text by highlighting
/// category (`Class`).
struct Classifier<'a> {
    tokens: Peekable<TokenIter<'a>>,
    in_attribute: bool,
    in_macro: bool,
    in_macro_nonterminal: bool,
}

impl<'a> Classifier<'a> {
    fn new(src: &str) -> Classifier<'_> {
        let tokens = TokenIter { src }.peekable();
        Classifier { tokens, in_attribute: false, in_macro: false, in_macro_nonterminal: false }
    }

    /// Exhausts the `Classifier` writing the output into `sink`.
    ///
    /// The general structure for this method is to iterate over each token,
    /// possibly giving it an HTML span with a class specifying what flavor of
    /// token is used.
    fn highlight(mut self, sink: &mut dyn FnMut(Highlight<'a>)) {
        with_default_session_globals(|| {
            while let Some((token, text)) = self.tokens.next() {
                self.advance(token, text, sink);
            }
        })
    }

    /// Single step of highlighting. This will classify `token`, but maybe also
    /// a couple of following ones as well.
    fn advance(&mut self, token: TokenKind, text: &'a str, sink: &mut dyn FnMut(Highlight<'a>)) {
        let lookahead = self.peek();
        let class = match token {
            TokenKind::Whitespace => Class::None,
            TokenKind::LineComment { doc_style } | TokenKind::BlockComment { doc_style, .. } => {
                if doc_style.is_some() {
                    Class::DocComment
                } else {
                    Class::Comment
                }
            }
            // Consider this as part of a macro invocation if there was a
            // leading identifier.
            TokenKind::Bang if self.in_macro => {
                self.in_macro = false;
                Class::Macro
            }

            // Assume that '&' or '*' is the reference or dereference operator
            // or a reference or pointer type. Unless, of course, it looks like
            // a logical and or a multiplication operator: `&&` or `* `.
            TokenKind::Star => match lookahead {
                Some(TokenKind::Whitespace) => Class::Op,
                _ => Class::RefKeyWord,
            },
            TokenKind::And => match lookahead {
                Some(TokenKind::And) => {
                    let _and = self.tokens.next();
                    sink(Highlight::Token { text: "&&", class: Class::Op });
                    return;
                }
                Some(TokenKind::Eq) => {
                    let _eq = self.tokens.next();
                    sink(Highlight::Token { text: "&=", class: Class::Op });
                    return;
                }
                Some(TokenKind::Whitespace) => Class::Op,
                _ => Class::RefKeyWord,
            },

            // Operators.
            TokenKind::Minus
            | TokenKind::Plus
            | TokenKind::Or
            | TokenKind::Slash
            | TokenKind::Caret
            | TokenKind::Percent
            | TokenKind::Bang
            | TokenKind::Eq
            | TokenKind::Lt
            | TokenKind::Gt => Class::Op,

            // Miscellaneous, no highlighting.
            TokenKind::Dot
            | TokenKind::Semi
            | TokenKind::Comma
            | TokenKind::OpenParen
            | TokenKind::CloseParen
            | TokenKind::OpenBrace
            | TokenKind::CloseBrace
            | TokenKind::OpenBracket
            | TokenKind::At
            | TokenKind::Tilde
            | TokenKind::Colon
            | TokenKind::Unknown => Class::None,

            TokenKind::Question => Class::QuestionMark,

            TokenKind::Dollar => match lookahead {
                Some(TokenKind::Ident) => {
                    self.in_macro_nonterminal = true;
                    Class::MacroNonTerminal
                }
                _ => Class::None,
            },

            // This might be the start of an attribute. We're going to want to
            // continue highlighting it as an attribute until the ending ']' is
            // seen, so skip out early. Down below we terminate the attribute
            // span when we see the ']'.
            TokenKind::Pound => {
                match lookahead {
                    // Case 1: #![inner_attribute]
                    Some(TokenKind::Bang) => {
                        let _not = self.tokens.next().unwrap();
                        if let Some(TokenKind::OpenBracket) = self.peek() {
                            self.in_attribute = true;
                            sink(Highlight::EnterSpan { class: Class::Attribute });
                        }
                        sink(Highlight::Token { text: "#", class: Class::None });
                        sink(Highlight::Token { text: "!", class: Class::None });
                        return;
                    }
                    // Case 2: #[outer_attribute]
                    Some(TokenKind::OpenBracket) => {
                        self.in_attribute = true;
                        sink(Highlight::EnterSpan { class: Class::Attribute });
                    }
                    _ => (),
                }
                Class::None
            }
            TokenKind::CloseBracket => {
                if self.in_attribute {
                    self.in_attribute = false;
                    sink(Highlight::Token { text: "]", class: Class::None });
                    sink(Highlight::ExitSpan);
                    return;
                }
                Class::None
            }
            TokenKind::Literal { kind, .. } => match kind {
                // Text literals.
                LiteralKind::Byte { .. }
                | LiteralKind::Char { .. }
                | LiteralKind::Str { .. }
                | LiteralKind::ByteStr { .. }
                | LiteralKind::RawStr { .. }
                | LiteralKind::RawByteStr { .. } => Class::String,
                // Number literals.
                LiteralKind::Float { .. } | LiteralKind::Int { .. } => Class::Number,
            },
            TokenKind::Ident | TokenKind::RawIdent if lookahead == Some(TokenKind::Bang) => {
                self.in_macro = true;
                Class::Macro
            }
            TokenKind::Ident => match text {
                "ref" | "mut" => Class::RefKeyWord,
                "self" | "Self" => Class::Self_,
                "false" | "true" => Class::Bool,
                "Option" | "Result" => Class::PreludeTy,
                "Some" | "None" | "Ok" | "Err" => Class::PreludeVal,
                // Keywords are also included in the identifier set.
                _ if Ident::from_str(text).is_reserved() => Class::KeyWord,
                _ if self.in_macro_nonterminal => {
                    self.in_macro_nonterminal = false;
                    Class::MacroNonTerminal
                }
                _ => Class::Ident,
            },
            TokenKind::RawIdent => Class::Ident,
            TokenKind::Lifetime { .. } => Class::Lifetime,
        };
        // Anything that didn't return above is the simple case where we the
        // class just spans a single token, so we can use the `string` method.
        sink(Highlight::Token { text, class });
    }

    fn peek(&mut self) -> Option<TokenKind> {
        self.tokens.peek().map(|(toke_kind, _text)| *toke_kind)
    }
}

/// Called when we start processing a span of text that should be highlighted.
/// The `Class` argument specifies how it should be highlighted.
fn enter_span(out: &mut String, klass: Class) {
    write!(out, "<span class=\"{}\">", klass.as_html()).unwrap()
}

/// Called at the end of a span of highlighted text.
fn exit_span(out: &mut String) {
    write!(out, "</span>").unwrap()
}

/// Called for a span of text. If the text should be highlighted differently
/// from the surrounding text, then the `Class` argument will be a value other
/// than `None`.
///
/// The following sequences of callbacks are equivalent:
/// ```plain
///     enter_span(Foo), string("text", None), exit_span()
///     string("text", Foo)
/// ```
/// The latter can be thought of as a shorthand for the former, which is more
/// flexible.
fn string<T: Display>(out: &mut String, text: T, klass: Class) {
    match klass {
        Class::None => write!(out, "{}", text).unwrap(),
        klass => write!(out, "<span class=\"{}\">{}</span>", klass.as_html(), text).unwrap(),
    }
}

#[cfg(test)]
mod tests;
