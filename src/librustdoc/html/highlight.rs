//! Basic syntax highlighting functionality.
//!
//! This module uses librustc_ast's lexer to provide token-based highlighting for
//! the HTML documentation generated by rustdoc.
//!
//! Use the `render_with_highlighting` to highlight some rust code.

use crate::html::escape::Escape;

use std::fmt::Display;
use std::iter::Peekable;

use rustc_lexer::{LiteralKind, TokenKind};
use rustc_span::edition::Edition;
use rustc_span::symbol::Symbol;

use super::format::Buffer;

/// Highlights `src`, returning the HTML output.
crate fn render_with_highlighting(
    src: &str,
    out: &mut Buffer,
    class: Option<&str>,
    playground_button: Option<&str>,
    tooltip: Option<(Option<Edition>, &str)>,
    edition: Edition,
) {
    debug!("highlighting: ================\n{}\n==============", src);
    if let Some((edition_info, class)) = tooltip {
        write!(
            out,
            "<div class='information'><div class='tooltip {}'{}>ⓘ</div></div>",
            class,
            if let Some(edition_info) = edition_info {
                format!(" data-edition=\"{}\"", edition_info)
            } else {
                String::new()
            },
        );
    }

    write_header(out, class);
    write_code(out, &src, edition);
    write_footer(out, playground_button);
}

fn write_header(out: &mut Buffer, class: Option<&str>) {
    writeln!(out, "<div class=\"example-wrap\"><pre class=\"rust {}\">", class.unwrap_or_default());
}

fn write_code(out: &mut Buffer, src: &str, edition: Edition) {
    // This replace allows to fix how the code source with DOS backline characters is displayed.
    let src = src.replace("\r\n", "\n");
    Classifier::new(&src, edition).highlight(&mut |highlight| {
        match highlight {
            Highlight::Token { text, class } => string(out, Escape(text), class),
            Highlight::EnterSpan { class } => enter_span(out, class),
            Highlight::ExitSpan => exit_span(out),
        };
    });
}

fn write_footer(out: &mut Buffer, playground_button: Option<&str>) {
    writeln!(out, "</pre>{}</div>", playground_button.unwrap_or_default());
}

/// How a span of text is classified. Mostly corresponds to token kinds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Class {
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
    Token { text: &'a str, class: Option<Class> },
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

fn get_real_ident_class(text: &str, edition: Edition) -> Class {
    match text {
        "ref" | "mut" => Class::RefKeyWord,
        "self" | "Self" => Class::Self_,
        "false" | "true" => Class::Bool,
        _ if Symbol::intern(text).is_reserved(|| edition) => Class::KeyWord,
        _ => Class::Ident,
    }
}

/// Processes program tokens, classifying strings of text by highlighting
/// category (`Class`).
struct Classifier<'a> {
    tokens: Peekable<TokenIter<'a>>,
    in_attribute: bool,
    in_macro: bool,
    in_macro_nonterminal: bool,
    edition: Edition,
    byte_pos: u32,
    src: &'a str,
}

impl<'a> Classifier<'a> {
    fn new(src: &str, edition: Edition) -> Classifier<'_> {
        let tokens = TokenIter { src }.peekable();
        Classifier {
            tokens,
            in_attribute: false,
            in_macro: false,
            in_macro_nonterminal: false,
            edition,
            byte_pos: 0,
            src,
        }
    }

    /// Concatenate colons and idents as one when possible.
    fn get_full_ident_path(&mut self) -> Vec<(TokenKind, usize, usize)> {
        let start = self.byte_pos as usize;
        let mut pos = start;
        let mut has_ident = false;
        let edition = self.edition;

        loop {
            let mut nb = 0;
            while let Some((TokenKind::Colon, _)) = self.tokens.peek() {
                self.tokens.next();
                nb += 1;
            }
            // Ident path can start with "::" but if we already have content in the ident path,
            // the "::" is mandatory.
            if has_ident && nb == 0 {
                return vec![(TokenKind::Ident, start, pos)];
            } else if nb != 0 && nb != 2 {
                if has_ident {
                    return vec![(TokenKind::Ident, start, pos), (TokenKind::Colon, pos, pos + nb)];
                } else {
                    return vec![(TokenKind::Colon, pos, pos + nb)];
                }
            }

            if let Some((Class::Ident, text)) = self.tokens.peek().map(|(token, text)| {
                if *token == TokenKind::Ident {
                    let class = get_real_ident_class(text, edition);
                    (class, text)
                } else {
                    // Doesn't matter which Class we put in here...
                    (Class::Comment, text)
                }
            }) {
                // We only "add" the colon if there is an ident behind.
                pos += text.len() + nb;
                has_ident = true;
                self.tokens.next();
            } else if nb > 0 && has_ident {
                return vec![(TokenKind::Ident, start, pos), (TokenKind::Colon, pos, pos + nb)];
            } else if nb > 0 {
                return vec![(TokenKind::Colon, pos, pos + nb)];
            } else if has_ident {
                return vec![(TokenKind::Ident, start, pos)];
            } else {
                return Vec::new();
            }
        }
    }

    /// Wraps the tokens iteration to ensure that the byte_pos is always correct.
    fn next(&mut self) -> Option<(TokenKind, &'a str)> {
        if let Some((kind, text)) = self.tokens.next() {
            self.byte_pos += text.len() as u32;
            Some((kind, text))
        } else {
            None
        }
    }

    /// Exhausts the `Classifier` writing the output into `sink`.
    ///
    /// The general structure for this method is to iterate over each token,
    /// possibly giving it an HTML span with a class specifying what flavor of
    /// token is used.
    fn highlight(mut self, sink: &mut dyn FnMut(Highlight<'a>)) {
        loop {
            if self
                .tokens
                .peek()
                .map(|t| matches!(t.0, TokenKind::Colon | TokenKind::Ident))
                .unwrap_or(false)
            {
                let tokens = self.get_full_ident_path();
                for (token, start, end) in tokens {
                    let text = &self.src[start..end];
                    self.advance(token, text, sink);
                    self.byte_pos += text.len() as u32;
                }
            }
            if let Some((token, text)) = self.next() {
                self.advance(token, text, sink);
            } else {
                break;
            }
        }
    }

    /// Single step of highlighting. This will classify `token`, but maybe also
    /// a couple of following ones as well.
    fn advance(&mut self, token: TokenKind, text: &'a str, sink: &mut dyn FnMut(Highlight<'a>)) {
        let lookahead = self.peek();
        let no_highlight = |sink: &mut dyn FnMut(_)| sink(Highlight::Token { text, class: None });
        let class = match token {
            TokenKind::Whitespace => return no_highlight(sink),
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
                sink(Highlight::Token { text, class: None });
                sink(Highlight::ExitSpan);
                return;
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
                    self.next();
                    sink(Highlight::Token { text: "&&", class: Some(Class::Op) });
                    return;
                }
                Some(TokenKind::Eq) => {
                    self.next();
                    sink(Highlight::Token { text: "&=", class: Some(Class::Op) });
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
            | TokenKind::Unknown => return no_highlight(sink),

            TokenKind::Question => Class::QuestionMark,

            TokenKind::Dollar => match lookahead {
                Some(TokenKind::Ident) => {
                    self.in_macro_nonterminal = true;
                    Class::MacroNonTerminal
                }
                _ => return no_highlight(sink),
            },

            // This might be the start of an attribute. We're going to want to
            // continue highlighting it as an attribute until the ending ']' is
            // seen, so skip out early. Down below we terminate the attribute
            // span when we see the ']'.
            TokenKind::Pound => {
                match lookahead {
                    // Case 1: #![inner_attribute]
                    Some(TokenKind::Bang) => {
                        self.next();
                        if let Some(TokenKind::OpenBracket) = self.peek() {
                            self.in_attribute = true;
                            sink(Highlight::EnterSpan { class: Class::Attribute });
                        }
                        sink(Highlight::Token { text: "#", class: None });
                        sink(Highlight::Token { text: "!", class: None });
                        return;
                    }
                    // Case 2: #[outer_attribute]
                    Some(TokenKind::OpenBracket) => {
                        self.in_attribute = true;
                        sink(Highlight::EnterSpan { class: Class::Attribute });
                    }
                    _ => (),
                }
                return no_highlight(sink);
            }
            TokenKind::CloseBracket => {
                if self.in_attribute {
                    self.in_attribute = false;
                    sink(Highlight::Token { text: "]", class: None });
                    sink(Highlight::ExitSpan);
                    return;
                }
                return no_highlight(sink);
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
                sink(Highlight::EnterSpan { class: Class::Macro });
                sink(Highlight::Token { text, class: None });
                return;
            }
            TokenKind::Ident => match get_real_ident_class(text, self.edition) {
                Class::Ident => match text {
                    "Option" | "Result" => Class::PreludeTy,
                    "Some" | "None" | "Ok" | "Err" => Class::PreludeVal,
                    _ if self.in_macro_nonterminal => {
                        self.in_macro_nonterminal = false;
                        Class::MacroNonTerminal
                    }
                    _ => Class::Ident,
                },
                c => c,
            },
            TokenKind::RawIdent => Class::Ident,
            TokenKind::Lifetime { .. } => Class::Lifetime,
        };
        // Anything that didn't return above is the simple case where we the
        // class just spans a single token, so we can use the `string` method.
        sink(Highlight::Token { text, class: Some(class) });
    }

    fn peek(&mut self) -> Option<TokenKind> {
        self.tokens.peek().map(|(toke_kind, _text)| *toke_kind)
    }
}

/// Called when we start processing a span of text that should be highlighted.
/// The `Class` argument specifies how it should be highlighted.
fn enter_span(out: &mut Buffer, klass: Class) {
    write!(out, "<span class=\"{}\">", klass.as_html());
}

/// Called at the end of a span of highlighted text.
fn exit_span(out: &mut Buffer) {
    out.write_str("</span>");
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
fn string<T: Display>(out: &mut Buffer, text: T, klass: Option<Class>) {
    match klass {
        None => write!(out, "{}", text),
        Some(klass) => write!(out, "<span class=\"{}\">{}</span>", klass.as_html(), text),
    }
}

#[cfg(test)]
mod tests;
