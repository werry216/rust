// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! # Quasiquoter
//! This file contains the implementation internals of the quasiquoter provided by `quote!`.

//! This quasiquoter uses macros 2.0 hygiene to reliably access
//! items from `proc_macro`, to build a `proc_macro::TokenStream`.

use {Delimiter, Literal, Spacing, Span, Ident, Punct, Group, TokenStream, TokenTree};

use syntax::ext::base::{ExtCtxt, ProcMacro};
use syntax::tokenstream;

/// This is the actual quote!() proc macro
///
/// It is manually loaded in CStore::load_macro_untracked
pub struct Quoter;

pub fn unquote<T: Into<TokenStream> + Clone>(tokens: &T) -> TokenStream {
    tokens.clone().into()
}

pub trait Quote {
    fn quote(self) -> TokenStream;
}

macro_rules! tt2ts {
    ($e:expr) => (TokenStream::from(TokenTree::from($e)))
}

macro_rules! quote_tok {
    (,) => { tt2ts!(Punct::new(',', Spacing::Alone)) };
    (.) => { tt2ts!(Punct::new('.', Spacing::Alone)) };
    (:) => { tt2ts!(Punct::new(':', Spacing::Alone)) };
    (;) => { tt2ts!(Punct::new(';', Spacing::Alone)) };
    (|) => { tt2ts!(Punct::new('|', Spacing::Alone)) };
    (::) => {
        [
            TokenTree::from(Punct::new(':', Spacing::Joint)),
            TokenTree::from(Punct::new(':', Spacing::Alone)),
        ].iter()
            .cloned()
            .map(|mut x| {
                x.set_span(Span::def_site());
                x
            })
            .collect::<TokenStream>()
    };
    (!) => { tt2ts!(Punct::new('!', Spacing::Alone)) };
    (<) => { tt2ts!(Punct::new('<', Spacing::Alone)) };
    (>) => { tt2ts!(Punct::new('>', Spacing::Alone)) };
    (_) => { tt2ts!(Punct::new('_', Spacing::Alone)) };
    (0) => { tt2ts!(Literal::i8_unsuffixed(0)) };
    (&) => { tt2ts!(Punct::new('&', Spacing::Alone)) };
    (=) => { tt2ts!(Punct::new('=', Spacing::Alone)) };
    ($i:ident) => { tt2ts!(Ident::new(stringify!($i), Span::def_site())) };
}

macro_rules! quote_tree {
    ((unquote $($t:tt)*)) => { $($t)* };
    ((quote $($t:tt)*)) => { ($($t)*).quote() };
    (($($t:tt)*)) => { tt2ts!(Group::new(Delimiter::Parenthesis, quote!($($t)*))) };
    ([$($t:tt)*]) => { tt2ts!(Group::new(Delimiter::Bracket, quote!($($t)*))) };
    ({$($t:tt)*}) => { tt2ts!(Group::new(Delimiter::Brace, quote!($($t)*))) };
    ($t:tt) => { quote_tok!($t) };
}

macro_rules! quote {
    () => { TokenStream::new() };
    ($($t:tt)*) => {
        [$(quote_tree!($t),)*].iter()
            .cloned()
            .flat_map(|x| x.into_iter())
            .collect::<TokenStream>()
    };
}

impl ProcMacro for Quoter {
    fn expand<'cx>(&self, cx: &'cx mut ExtCtxt,
                   _: ::syntax_pos::Span,
                   stream: tokenstream::TokenStream)
                   -> tokenstream::TokenStream {
        ::__internal::set_sess(cx, || TokenStream(stream).quote().0)
    }
}

impl Quote for TokenStream {
    fn quote(self) -> TokenStream {
        if self.is_empty() {
            return quote!(::TokenStream::new());
        }
        let mut after_dollar = false;
        let tokens = self.into_iter().filter_map(|tree| {
            if after_dollar {
                after_dollar = false;
                match tree {
                    TokenTree::Ident(_) => {
                        let tree = TokenStream::from(tree);
                        return Some(quote!(::__internal::unquote(&(unquote tree)),));
                    }
                    TokenTree::Punct(ref tt) if tt.as_char() == '$' => {}
                    _ => panic!("`$` must be followed by an ident or `$` in `quote!`"),
                }
            } else if let TokenTree::Punct(ref tt) = tree {
                if tt.as_char() == '$' {
                    after_dollar = true;
                    return None;
                }
            }

            Some(quote!(::TokenStream::from((quote tree)),))
        }).flat_map(|t| t.into_iter()).collect::<TokenStream>();

        if after_dollar {
            panic!("unexpected trailing `$` in `quote!`");
        }

        quote!(
            [(unquote tokens)].iter()
                .cloned()
                .flat_map(|x| x.into_iter())
                .collect::<::TokenStream>()
        )
    }
}

impl Quote for TokenTree {
    fn quote(self) -> TokenStream {
        match self {
            TokenTree::Punct(tt) => quote!(::TokenTree::Punct( (quote tt) )),
            TokenTree::Group(tt) => quote!(::TokenTree::Group( (quote tt) )),
            TokenTree::Ident(tt) => quote!(::TokenTree::Ident( (quote tt) )),
            TokenTree::Literal(tt) => quote!(::TokenTree::Literal( (quote tt) )),
        }
    }
}

impl Quote for char {
    fn quote(self) -> TokenStream {
        TokenTree::from(Literal::character(self)).into()
    }
}

impl<'a> Quote for &'a str {
    fn quote(self) -> TokenStream {
        TokenTree::from(Literal::string(self)).into()
    }
}

impl Quote for u16 {
    fn quote(self) -> TokenStream {
        TokenTree::from(Literal::u16_unsuffixed(self)).into()
    }
}

impl Quote for Group {
    fn quote(self) -> TokenStream {
        quote!(::Group::new((quote self.delimiter()), (quote self.stream())))
    }
}

impl Quote for Punct {
    fn quote(self) -> TokenStream {
        quote!(::Punct::new((quote self.as_char()), (quote self.spacing())))
    }
}

impl Quote for Ident {
    fn quote(self) -> TokenStream {
        quote!(::Ident::new((quote self.sym.as_str()), (quote self.span())))
    }
}

impl Quote for Span {
    fn quote(self) -> TokenStream {
        quote!(::Span::def_site())
    }
}

impl Quote for Literal {
    fn quote(self) -> TokenStream {
        quote! {{
            let mut iter = (quote self.to_string())
                .parse::<::TokenStream>()
                .unwrap()
                .into_iter();
            if let (Some(::TokenTree::Literal(mut lit)), None) = (iter.next(), iter.next()) {
                lit.set_span((quote self.span));
                lit
            } else {
                unreachable!()
            }
        }}
    }
}

impl Quote for Delimiter {
    fn quote(self) -> TokenStream {
        macro_rules! gen_match {
            ($($i:ident),*) => {
                match self {
                    $(Delimiter::$i => { quote!(::Delimiter::$i) })*
                }
            }
        }

        gen_match!(Parenthesis, Brace, Bracket, None)
    }
}

impl Quote for Spacing {
    fn quote(self) -> TokenStream {
        macro_rules! gen_match {
            ($($i:ident),*) => {
                match self {
                    $(Spacing::$i => { quote!(::Spacing::$i) })*
                }
            }
        }

        gen_match!(Alone, Joint)
    }
}
