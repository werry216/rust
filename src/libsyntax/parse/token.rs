pub use BinOpToken::*;
pub use Nonterminal::*;
pub use DelimToken::*;
pub use Lit::*;
pub use Token::*;

use crate::ast::{self};
use crate::parse::ParseSess;
use crate::print::pprust;
use crate::ptr::P;
use crate::symbol::keywords;
use crate::syntax::parse::parse_stream_from_source_str;
use crate::tokenstream::{self, DelimSpan, TokenStream, TokenTree};

use syntax_pos::symbol::{self, Symbol};
use syntax_pos::{self, Span, FileName};
use log::info;

use std::fmt;
use std::mem;
#[cfg(target_arch = "x86_64")]
use rustc_data_structures::static_assert;
use rustc_data_structures::sync::Lrc;

#[derive(Clone, PartialEq, RustcEncodable, RustcDecodable, Hash, Debug, Copy)]
pub enum BinOpToken {
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    And,
    Or,
    Shl,
    Shr,
}

/// A delimiter token.
#[derive(Clone, PartialEq, RustcEncodable, RustcDecodable, Hash, Debug, Copy)]
pub enum DelimToken {
    /// A round parenthesis (i.e., `(` or `)`).
    Paren,
    /// A square bracket (i.e., `[` or `]`).
    Bracket,
    /// A curly brace (i.e., `{` or `}`).
    Brace,
    /// An empty delimiter.
    NoDelim,
}

impl DelimToken {
    pub fn len(self) -> usize {
        if self == NoDelim { 0 } else { 1 }
    }

    pub fn is_empty(self) -> bool {
        self == NoDelim
    }
}

#[derive(Clone, PartialEq, RustcEncodable, RustcDecodable, Hash, Debug, Copy)]
pub enum Lit {
    Byte(ast::Name),
    Char(ast::Name),
    Err(ast::Name),
    Integer(ast::Name),
    Float(ast::Name),
    Str_(ast::Name),
    StrRaw(ast::Name, u16), /* raw str delimited by n hash symbols */
    ByteStr(ast::Name),
    ByteStrRaw(ast::Name, u16), /* raw byte str delimited by n hash symbols */
}

impl Lit {
    crate fn literal_name(&self) -> &'static str {
        match *self {
            Byte(_) => "byte literal",
            Char(_) => "char literal",
            Err(_) => "invalid literal",
            Integer(_) => "integer literal",
            Float(_) => "float literal",
            Str_(_) | StrRaw(..) => "string literal",
            ByteStr(_) | ByteStrRaw(..) => "byte string literal"
        }
    }

    // See comments in `Nonterminal::to_tokenstream` for why we care about
    // *probably* equal here rather than actual equality
    fn probably_equal_for_proc_macro(&self, other: &Lit) -> bool {
        mem::discriminant(self) == mem::discriminant(other)
    }
}

pub(crate) fn ident_can_begin_expr(ident: ast::Ident, is_raw: bool) -> bool {
    let ident_token: Token = Ident(ident, is_raw);

    !ident_token.is_reserved_ident() ||
    ident_token.is_path_segment_keyword() ||
    [
        keywords::Async.name(),

        // FIXME: remove when `await!(..)` syntax is removed
        // https://github.com/rust-lang/rust/issues/60610
        keywords::Await.name(),

        keywords::Do.name(),
        keywords::Box.name(),
        keywords::Break.name(),
        keywords::Continue.name(),
        keywords::False.name(),
        keywords::For.name(),
        keywords::If.name(),
        keywords::Loop.name(),
        keywords::Match.name(),
        keywords::Move.name(),
        keywords::Return.name(),
        keywords::True.name(),
        keywords::Unsafe.name(),
        keywords::While.name(),
        keywords::Yield.name(),
        keywords::Static.name(),
    ].contains(&ident.name)
}

fn ident_can_begin_type(ident: ast::Ident, is_raw: bool) -> bool {
    let ident_token: Token = Ident(ident, is_raw);

    !ident_token.is_reserved_ident() ||
    ident_token.is_path_segment_keyword() ||
    [
        keywords::Underscore.name(),
        keywords::For.name(),
        keywords::Impl.name(),
        keywords::Fn.name(),
        keywords::Unsafe.name(),
        keywords::Extern.name(),
        keywords::Typeof.name(),
        keywords::Dyn.name(),
    ].contains(&ident.name)
}

#[derive(Clone, RustcEncodable, RustcDecodable, PartialEq, Debug)]
pub enum Token {
    /* Expression-operator symbols. */
    Eq,
    Lt,
    Le,
    EqEq,
    Ne,
    Ge,
    Gt,
    AndAnd,
    OrOr,
    Not,
    Tilde,
    BinOp(BinOpToken),
    BinOpEq(BinOpToken),

    /* Structural symbols */
    At,
    Dot,
    DotDot,
    DotDotDot,
    DotDotEq,
    Comma,
    Semi,
    Colon,
    ModSep,
    RArrow,
    LArrow,
    FatArrow,
    Pound,
    Dollar,
    Question,
    /// Used by proc macros for representing lifetimes, not generated by lexer right now.
    SingleQuote,
    /// An opening delimiter (e.g., `{`).
    OpenDelim(DelimToken),
    /// A closing delimiter (e.g., `}`).
    CloseDelim(DelimToken),

    /* Literals */
    Literal(Lit, Option<ast::Name>),

    /* Name components */
    Ident(ast::Ident, /* is_raw */ bool),
    Lifetime(ast::Ident),

    Interpolated(Lrc<Nonterminal>),

    // Can be expanded into several tokens.
    /// A doc comment.
    DocComment(ast::Name),

    // Junk. These carry no data because we don't really care about the data
    // they *would* carry, and don't really want to allocate a new ident for
    // them. Instead, users could extract that from the associated span.

    /// Whitespace.
    Whitespace,
    /// A comment.
    Comment,
    Shebang(ast::Name),

    Eof,
}

// `Token` is used a lot. Make sure it doesn't unintentionally get bigger.
#[cfg(target_arch = "x86_64")]
static_assert!(MEM_SIZE_OF_STATEMENT: mem::size_of::<Token>() == 16);

impl Token {
    /// Recovers a `Token` from an `ast::Ident`. This creates a raw identifier if necessary.
    pub fn from_ast_ident(ident: ast::Ident) -> Token {
        Ident(ident, ident.is_raw_guess())
    }

    crate fn is_like_plus(&self) -> bool {
        match *self {
            BinOp(Plus) | BinOpEq(Plus) => true,
            _ => false,
        }
    }

    /// Returns `true` if the token can appear at the start of an expression.
    crate fn can_begin_expr(&self) -> bool {
        match *self {
            Ident(ident, is_raw)              =>
                ident_can_begin_expr(ident, is_raw), // value name or keyword
            OpenDelim(..)                     | // tuple, array or block
            Literal(..)                       | // literal
            Not                               | // operator not
            BinOp(Minus)                      | // unary minus
            BinOp(Star)                       | // dereference
            BinOp(Or) | OrOr                  | // closure
            BinOp(And)                        | // reference
            AndAnd                            | // double reference
            // DotDotDot is no longer supported, but we need some way to display the error
            DotDot | DotDotDot | DotDotEq     | // range notation
            Lt | BinOp(Shl)                   | // associated path
            ModSep                            | // global path
            Lifetime(..)                      | // labeled loop
            Pound                             => true, // expression attributes
            Interpolated(ref nt) => match **nt {
                NtLiteral(..) |
                NtIdent(..)   |
                NtExpr(..)    |
                NtBlock(..)   |
                NtPath(..)    |
                NtLifetime(..) => true,
                _ => false,
            },
            _ => false,
        }
    }

    /// Returns `true` if the token can appear at the start of a type.
    crate fn can_begin_type(&self) -> bool {
        match *self {
            Ident(ident, is_raw)        =>
                ident_can_begin_type(ident, is_raw), // type name or keyword
            OpenDelim(Paren)            | // tuple
            OpenDelim(Bracket)          | // array
            Not                         | // never
            BinOp(Star)                 | // raw pointer
            BinOp(And)                  | // reference
            AndAnd                      | // double reference
            Question                    | // maybe bound in trait object
            Lifetime(..)                | // lifetime bound in trait object
            Lt | BinOp(Shl)             | // associated path
            ModSep                      => true, // global path
            Interpolated(ref nt) => match **nt {
                NtIdent(..) | NtTy(..) | NtPath(..) | NtLifetime(..) => true,
                _ => false,
            },
            _ => false,
        }
    }

    /// Returns `true` if the token can appear at the start of a const param.
    pub fn can_begin_const_arg(&self) -> bool {
        match self {
            OpenDelim(Brace) => true,
            Interpolated(ref nt) => match **nt {
                NtExpr(..) => true,
                NtBlock(..) => true,
                NtLiteral(..) => true,
                _ => false,
            }
            _ => self.can_begin_literal_or_bool(),
        }
    }

    /// Returns `true` if the token can appear at the start of a generic bound.
    crate fn can_begin_bound(&self) -> bool {
        self.is_path_start() || self.is_lifetime() || self.is_keyword(keywords::For) ||
        self == &Question || self == &OpenDelim(Paren)
    }

    /// Returns `true` if the token is any literal
    crate fn is_lit(&self) -> bool {
        match *self {
            Literal(..) => true,
            _           => false,
        }
    }

    /// Returns `true` if the token is any literal, a minus (which can prefix a literal,
    /// for example a '-42', or one of the boolean idents).
    crate fn can_begin_literal_or_bool(&self) -> bool {
        match *self {
            Literal(..)  => true,
            BinOp(Minus) => true,
            Ident(ident, false) if ident.name == keywords::True.name() => true,
            Ident(ident, false) if ident.name == keywords::False.name() => true,
            Interpolated(ref nt) => match **nt {
                NtLiteral(..) => true,
                _             => false,
            },
            _            => false,
        }
    }

    /// Returns an identifier if this token is an identifier.
    pub fn ident(&self) -> Option<(ast::Ident, /* is_raw */ bool)> {
        match *self {
            Ident(ident, is_raw) => Some((ident, is_raw)),
            Interpolated(ref nt) => match **nt {
                NtIdent(ident, is_raw) => Some((ident, is_raw)),
                _ => None,
            },
            _ => None,
        }
    }
    /// Returns a lifetime identifier if this token is a lifetime.
    pub fn lifetime(&self) -> Option<ast::Ident> {
        match *self {
            Lifetime(ident) => Some(ident),
            Interpolated(ref nt) => match **nt {
                NtLifetime(ident) => Some(ident),
                _ => None,
            },
            _ => None,
        }
    }
    /// Returns `true` if the token is an identifier.
    pub fn is_ident(&self) -> bool {
        self.ident().is_some()
    }
    /// Returns `true` if the token is a lifetime.
    crate fn is_lifetime(&self) -> bool {
        self.lifetime().is_some()
    }

    /// Returns `true` if the token is a identifier whose name is the given
    /// string slice.
    crate fn is_ident_named(&self, name: &str) -> bool {
        match self.ident() {
            Some((ident, _)) => ident.as_str() == name,
            None => false
        }
    }

    /// Returns `true` if the token is an interpolated path.
    fn is_path(&self) -> bool {
        if let Interpolated(ref nt) = *self {
            if let NtPath(..) = **nt {
                return true;
            }
        }
        false
    }

    /// Returns `true` if the token is either the `mut` or `const` keyword.
    crate fn is_mutability(&self) -> bool {
        self.is_keyword(keywords::Mut) ||
        self.is_keyword(keywords::Const)
    }

    crate fn is_qpath_start(&self) -> bool {
        self == &Lt || self == &BinOp(Shl)
    }

    crate fn is_path_start(&self) -> bool {
        self == &ModSep || self.is_qpath_start() || self.is_path() ||
        self.is_path_segment_keyword() || self.is_ident() && !self.is_reserved_ident()
    }

    /// Returns `true` if the token is a given keyword, `kw`.
    pub fn is_keyword(&self, kw: keywords::Keyword) -> bool {
        self.ident().map(|(ident, is_raw)| ident.name == kw.name() && !is_raw).unwrap_or(false)
    }

    pub fn is_path_segment_keyword(&self) -> bool {
        match self.ident() {
            Some((id, false)) => id.is_path_segment_keyword(),
            _ => false,
        }
    }

    // Returns true for reserved identifiers used internally for elided lifetimes,
    // unnamed method parameters, crate root module, error recovery etc.
    pub fn is_special_ident(&self) -> bool {
        match self.ident() {
            Some((id, false)) => id.is_special(),
            _ => false,
        }
    }

    /// Returns `true` if the token is a keyword used in the language.
    crate fn is_used_keyword(&self) -> bool {
        match self.ident() {
            Some((id, false)) => id.is_used_keyword(),
            _ => false,
        }
    }

    /// Returns `true` if the token is a keyword reserved for possible future use.
    crate fn is_unused_keyword(&self) -> bool {
        match self.ident() {
            Some((id, false)) => id.is_unused_keyword(),
            _ => false,
        }
    }

    /// Returns `true` if the token is either a special identifier or a keyword.
    pub fn is_reserved_ident(&self) -> bool {
        match self.ident() {
            Some((id, false)) => id.is_reserved(),
            _ => false,
        }
    }

    crate fn glue(self, joint: Token) -> Option<Token> {
        Some(match self {
            Eq => match joint {
                Eq => EqEq,
                Gt => FatArrow,
                _ => return None,
            },
            Lt => match joint {
                Eq => Le,
                Lt => BinOp(Shl),
                Le => BinOpEq(Shl),
                BinOp(Minus) => LArrow,
                _ => return None,
            },
            Gt => match joint {
                Eq => Ge,
                Gt => BinOp(Shr),
                Ge => BinOpEq(Shr),
                _ => return None,
            },
            Not => match joint {
                Eq => Ne,
                _ => return None,
            },
            BinOp(op) => match joint {
                Eq => BinOpEq(op),
                BinOp(And) if op == And => AndAnd,
                BinOp(Or) if op == Or => OrOr,
                Gt if op == Minus => RArrow,
                _ => return None,
            },
            Dot => match joint {
                Dot => DotDot,
                DotDot => DotDotDot,
                _ => return None,
            },
            DotDot => match joint {
                Dot => DotDotDot,
                Eq => DotDotEq,
                _ => return None,
            },
            Colon => match joint {
                Colon => ModSep,
                _ => return None,
            },
            SingleQuote => match joint {
                Ident(ident, false) => {
                    let name = Symbol::intern(&format!("'{}", ident));
                    Lifetime(symbol::Ident {
                        name,
                        span: ident.span,
                    })
                }
                _ => return None,
            },

            Le | EqEq | Ne | Ge | AndAnd | OrOr | Tilde | BinOpEq(..) | At | DotDotDot |
            DotDotEq | Comma | Semi | ModSep | RArrow | LArrow | FatArrow | Pound | Dollar |
            Question | OpenDelim(..) | CloseDelim(..) |
            Literal(..) | Ident(..) | Lifetime(..) | Interpolated(..) | DocComment(..) |
            Whitespace | Comment | Shebang(..) | Eof => return None,
        })
    }

    /// Returns tokens that are likely to be typed accidentally instead of the current token.
    /// Enables better error recovery when the wrong token is found.
    crate fn similar_tokens(&self) -> Option<Vec<Token>> {
        match *self {
            Comma => Some(vec![Dot, Lt, Semi]),
            Semi => Some(vec![Colon, Comma]),
            _ => None
        }
    }

    // See comments in `Nonterminal::to_tokenstream` for why we care about
    // *probably* equal here rather than actual equality
    crate fn probably_equal_for_proc_macro(&self, other: &Token) -> bool {
        if mem::discriminant(self) != mem::discriminant(other) {
            return false
        }
        match (self, other) {
            (&Eq, &Eq) |
            (&Lt, &Lt) |
            (&Le, &Le) |
            (&EqEq, &EqEq) |
            (&Ne, &Ne) |
            (&Ge, &Ge) |
            (&Gt, &Gt) |
            (&AndAnd, &AndAnd) |
            (&OrOr, &OrOr) |
            (&Not, &Not) |
            (&Tilde, &Tilde) |
            (&At, &At) |
            (&Dot, &Dot) |
            (&DotDot, &DotDot) |
            (&DotDotDot, &DotDotDot) |
            (&DotDotEq, &DotDotEq) |
            (&Comma, &Comma) |
            (&Semi, &Semi) |
            (&Colon, &Colon) |
            (&ModSep, &ModSep) |
            (&RArrow, &RArrow) |
            (&LArrow, &LArrow) |
            (&FatArrow, &FatArrow) |
            (&Pound, &Pound) |
            (&Dollar, &Dollar) |
            (&Question, &Question) |
            (&Whitespace, &Whitespace) |
            (&Comment, &Comment) |
            (&Eof, &Eof) => true,

            (&BinOp(a), &BinOp(b)) |
            (&BinOpEq(a), &BinOpEq(b)) => a == b,

            (&OpenDelim(a), &OpenDelim(b)) |
            (&CloseDelim(a), &CloseDelim(b)) => a == b,

            (&DocComment(a), &DocComment(b)) |
            (&Shebang(a), &Shebang(b)) => a == b,

            (&Lifetime(a), &Lifetime(b)) => a.name == b.name,
            (&Ident(a, b), &Ident(c, d)) => b == d && (a.name == c.name ||
                                                       a.name == keywords::DollarCrate.name() ||
                                                       c.name == keywords::DollarCrate.name()),

            (&Literal(ref a, b), &Literal(ref c, d)) => {
                b == d && a.probably_equal_for_proc_macro(c)
            }

            (&Interpolated(_), &Interpolated(_)) => false,

            _ => panic!("forgot to add a token?"),
        }
    }
}

#[derive(Clone, RustcEncodable, RustcDecodable)]
/// For interpolation during macro expansion.
pub enum Nonterminal {
    NtItem(P<ast::Item>),
    NtBlock(P<ast::Block>),
    NtStmt(ast::Stmt),
    NtPat(P<ast::Pat>),
    NtExpr(P<ast::Expr>),
    NtTy(P<ast::Ty>),
    NtIdent(ast::Ident, /* is_raw */ bool),
    NtLifetime(ast::Ident),
    NtLiteral(P<ast::Expr>),
    /// Stuff inside brackets for attributes
    NtMeta(ast::MetaItem),
    NtPath(ast::Path),
    NtVis(ast::Visibility),
    NtTT(TokenTree),
    // These are not exposed to macros, but are used by quasiquote.
    NtArm(ast::Arm),
    NtImplItem(ast::ImplItem),
    NtTraitItem(ast::TraitItem),
    NtForeignItem(ast::ForeignItem),
    NtGenerics(ast::Generics),
    NtWhereClause(ast::WhereClause),
    NtArg(ast::Arg),
}

impl PartialEq for Nonterminal {
    fn eq(&self, rhs: &Self) -> bool {
        match (self, rhs) {
            (NtIdent(ident_lhs, is_raw_lhs), NtIdent(ident_rhs, is_raw_rhs)) =>
                ident_lhs == ident_rhs && is_raw_lhs == is_raw_rhs,
            (NtLifetime(ident_lhs), NtLifetime(ident_rhs)) => ident_lhs == ident_rhs,
            (NtTT(tt_lhs), NtTT(tt_rhs)) => tt_lhs == tt_rhs,
            // FIXME: Assume that all "complex" nonterminal are not equal, we can't compare them
            // correctly based on data from AST. This will prevent them from matching each other
            // in macros. The comparison will become possible only when each nonterminal has an
            // attached token stream from which it was parsed.
            _ => false,
        }
    }
}

impl fmt::Debug for Nonterminal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            NtItem(..) => f.pad("NtItem(..)"),
            NtBlock(..) => f.pad("NtBlock(..)"),
            NtStmt(..) => f.pad("NtStmt(..)"),
            NtPat(..) => f.pad("NtPat(..)"),
            NtExpr(..) => f.pad("NtExpr(..)"),
            NtTy(..) => f.pad("NtTy(..)"),
            NtIdent(..) => f.pad("NtIdent(..)"),
            NtLiteral(..) => f.pad("NtLiteral(..)"),
            NtMeta(..) => f.pad("NtMeta(..)"),
            NtPath(..) => f.pad("NtPath(..)"),
            NtTT(..) => f.pad("NtTT(..)"),
            NtArm(..) => f.pad("NtArm(..)"),
            NtImplItem(..) => f.pad("NtImplItem(..)"),
            NtTraitItem(..) => f.pad("NtTraitItem(..)"),
            NtForeignItem(..) => f.pad("NtForeignItem(..)"),
            NtGenerics(..) => f.pad("NtGenerics(..)"),
            NtWhereClause(..) => f.pad("NtWhereClause(..)"),
            NtArg(..) => f.pad("NtArg(..)"),
            NtVis(..) => f.pad("NtVis(..)"),
            NtLifetime(..) => f.pad("NtLifetime(..)"),
        }
    }
}

impl Nonterminal {
    pub fn to_tokenstream(&self, sess: &ParseSess, span: Span) -> TokenStream {
        // A `Nonterminal` is often a parsed AST item. At this point we now
        // need to convert the parsed AST to an actual token stream, e.g.
        // un-parse it basically.
        //
        // Unfortunately there's not really a great way to do that in a
        // guaranteed lossless fashion right now. The fallback here is to just
        // stringify the AST node and reparse it, but this loses all span
        // information.
        //
        // As a result, some AST nodes are annotated with the token stream they
        // came from. Here we attempt to extract these lossless token streams
        // before we fall back to the stringification.
        let tokens = match *self {
            Nonterminal::NtItem(ref item) => {
                prepend_attrs(sess, &item.attrs, item.tokens.as_ref(), span)
            }
            Nonterminal::NtTraitItem(ref item) => {
                prepend_attrs(sess, &item.attrs, item.tokens.as_ref(), span)
            }
            Nonterminal::NtImplItem(ref item) => {
                prepend_attrs(sess, &item.attrs, item.tokens.as_ref(), span)
            }
            Nonterminal::NtIdent(ident, is_raw) => {
                let token = Token::Ident(ident, is_raw);
                Some(TokenTree::Token(ident.span, token).into())
            }
            Nonterminal::NtLifetime(ident) => {
                let token = Token::Lifetime(ident);
                Some(TokenTree::Token(ident.span, token).into())
            }
            Nonterminal::NtTT(ref tt) => {
                Some(tt.clone().into())
            }
            _ => None,
        };

        // FIXME(#43081): Avoid this pretty-print + reparse hack
        let source = pprust::nonterminal_to_string(self);
        let filename = FileName::macro_expansion_source_code(&source);
        let tokens_for_real = parse_stream_from_source_str(filename, source, sess, Some(span));

        // During early phases of the compiler the AST could get modified
        // directly (e.g., attributes added or removed) and the internal cache
        // of tokens my not be invalidated or updated. Consequently if the
        // "lossless" token stream disagrees with our actual stringification
        // (which has historically been much more battle-tested) then we go
        // with the lossy stream anyway (losing span information).
        //
        // Note that the comparison isn't `==` here to avoid comparing spans,
        // but it *also* is a "probable" equality which is a pretty weird
        // definition. We mostly want to catch actual changes to the AST
        // like a `#[cfg]` being processed or some weird `macro_rules!`
        // expansion.
        //
        // What we *don't* want to catch is the fact that a user-defined
        // literal like `0xf` is stringified as `15`, causing the cached token
        // stream to not be literal `==` token-wise (ignoring spans) to the
        // token stream we got from stringification.
        //
        // Instead the "probably equal" check here is "does each token
        // recursively have the same discriminant?" We basically don't look at
        // the token values here and assume that such fine grained token stream
        // modifications, including adding/removing typically non-semantic
        // tokens such as extra braces and commas, don't happen.
        if let Some(tokens) = tokens {
            if tokens.probably_equal_for_proc_macro(&tokens_for_real) {
                return tokens
            }
            info!("cached tokens found, but they're not \"probably equal\", \
                   going with stringified version");
        }
        return tokens_for_real
    }
}

crate fn is_op(tok: &Token) -> bool {
    match *tok {
        OpenDelim(..) | CloseDelim(..) | Literal(..) | DocComment(..) |
        Ident(..) | Lifetime(..) | Interpolated(..) |
        Whitespace | Comment | Shebang(..) | Eof => false,
        _ => true,
    }
}

fn prepend_attrs(sess: &ParseSess,
                 attrs: &[ast::Attribute],
                 tokens: Option<&tokenstream::TokenStream>,
                 span: syntax_pos::Span)
    -> Option<tokenstream::TokenStream>
{
    let tokens = tokens?;
    if attrs.len() == 0 {
        return Some(tokens.clone())
    }
    let mut builder = tokenstream::TokenStreamBuilder::new();
    for attr in attrs {
        assert_eq!(attr.style, ast::AttrStyle::Outer,
                   "inner attributes should prevent cached tokens from existing");

        let source = pprust::attr_to_string(attr);
        let macro_filename = FileName::macro_expansion_source_code(&source);
        if attr.is_sugared_doc {
            let stream = parse_stream_from_source_str(macro_filename, source, sess, Some(span));
            builder.push(stream);
            continue
        }

        // synthesize # [ $path $tokens ] manually here
        let mut brackets = tokenstream::TokenStreamBuilder::new();

        // For simple paths, push the identifier directly
        if attr.path.segments.len() == 1 && attr.path.segments[0].args.is_none() {
            let ident = attr.path.segments[0].ident;
            let token = Ident(ident, ident.as_str().starts_with("r#"));
            brackets.push(tokenstream::TokenTree::Token(ident.span, token));

        // ... and for more complicated paths, fall back to a reparse hack that
        // should eventually be removed.
        } else {
            let stream = parse_stream_from_source_str(macro_filename, source, sess, Some(span));
            brackets.push(stream);
        }

        brackets.push(attr.tokens.clone());

        // The span we list here for `#` and for `[ ... ]` are both wrong in
        // that it encompasses more than each token, but it hopefully is "good
        // enough" for now at least.
        builder.push(tokenstream::TokenTree::Token(attr.span, Pound));
        let delim_span = DelimSpan::from_single(attr.span);
        builder.push(tokenstream::TokenTree::Delimited(
            delim_span, DelimToken::Bracket, brackets.build().into()));
    }
    builder.push(tokens.clone());
    Some(builder.build())
}
