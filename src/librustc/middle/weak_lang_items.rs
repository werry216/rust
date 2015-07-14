// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Validity checking for weak lang items

use session::config;
use session::Session;
use metadata::csearch;
use middle::lang_items;

use syntax::ast;
use syntax::codemap::Span;
use syntax::parse::token::InternedString;
use syntax::visit::Visitor;
use syntax::visit;

use std::collections::HashSet;

macro_rules! weak_lang_items {
    ($($name:ident, $item:ident, $sym:ident;)*) => (

struct Context<'a> {
    sess: &'a Session,
    items: &'a mut lang_items::LanguageItems,
}

/// Checks the crate for usage of weak lang items, returning a vector of all the
/// language items required by this crate, but not defined yet.
pub fn check_crate(krate: &ast::Crate,
                   sess: &Session,
                   items: &mut lang_items::LanguageItems) {
    // These are never called by user code, they're generated by the compiler.
    // They will never implicitly be added to the `missing` array unless we do
    // so here.
    if items.stack_exhausted().is_none() {
        items.missing.push(lang_items::StackExhaustedLangItem);
    }
    if items.eh_personality().is_none() {
        items.missing.push(lang_items::EhPersonalityLangItem);
    }
    if sess.target.target.options.custom_unwind_resume &
       items.eh_unwind_resume().is_none() {
        items.missing.push(lang_items::EhUnwindResumeLangItem);
    }

    {
        let mut cx = Context { sess: sess, items: items };
        visit::walk_crate(&mut cx, krate);
    }
    verify(sess, items);
}

pub fn link_name(attrs: &[ast::Attribute]) -> Option<InternedString> {
    lang_items::extract(attrs).and_then(|name| {
        $(if &name[..] == stringify!($name) {
            Some(InternedString::new(stringify!($sym)))
        } else)* {
            None
        }
    })
}

fn verify(sess: &Session, items: &lang_items::LanguageItems) {
    // We only need to check for the presence of weak lang items if we're
    // emitting something that's not an rlib.
    let needs_check = sess.crate_types.borrow().iter().any(|kind| {
        match *kind {
            config::CrateTypeDylib |
            config::CrateTypeExecutable |
            config::CrateTypeStaticlib => true,
            config::CrateTypeRlib => false,
        }
    });
    if !needs_check { return }

    let mut missing = HashSet::new();
    sess.cstore.iter_crate_data(|cnum, _| {
        for item in &csearch::get_missing_lang_items(&sess.cstore, cnum) {
            missing.insert(*item);
        }
    });

    $(
        if missing.contains(&lang_items::$item) && items.$name().is_none() {
            sess.err(&format!("language item required, but not found: `{}`",
                              stringify!($name)));

        }
    )*
}

impl<'a> Context<'a> {
    fn register(&mut self, name: &str, span: Span) {
        $(if name == stringify!($name) {
            if self.items.$name().is_none() {
                self.items.missing.push(lang_items::$item);
            }
        } else)* {
            span_err!(self.sess, span, E0264,
                               "unknown external lang item: `{}`",
                                       name);
        }
    }
}

impl<'a, 'v> Visitor<'v> for Context<'a> {
    fn visit_foreign_item(&mut self, i: &ast::ForeignItem) {
        match lang_items::extract(&i.attrs) {
            None => {}
            Some(lang_item) => self.register(&lang_item, i.span),
        }
        visit::walk_foreign_item(self, i)
    }
}

) }

weak_lang_items! {
    panic_fmt,          PanicFmtLangItem,           rust_begin_unwind;
    stack_exhausted,    StackExhaustedLangItem,     rust_stack_exhausted;
    eh_personality,     EhPersonalityLangItem,      rust_eh_personality;
    eh_unwind_resume,   EhUnwindResumeLangItem,     rust_eh_unwind_resume;
}
