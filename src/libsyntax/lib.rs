// Copyright 2012-2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*! This module contains the Rust parser. It maps source text
 *  to token trees and to ASTs. It contains code for expanding
 *  macros.
 */

#[pkgid = "syntax#0.9-pre"];
#[license = "MIT/ASL2"];
#[crate_type = "dylib"];
#[crate_type = "rlib"];

#[feature(macro_rules, globs, managed_boxes)];

extern mod extra;

pub mod util {
    pub mod interner;
    #[cfg(test)]
    pub mod parser_testing;
    pub mod small_vector;
}

pub mod syntax {
    pub use ext;
    pub use parse;
}

pub mod opt_vec;
pub mod attr;
pub mod diagnostic;
pub mod codemap;
pub mod abi;
pub mod ast;
pub mod ast_util;
pub mod ast_map;
pub mod visit;
pub mod fold;


pub mod parse;
pub mod pkgid;

pub mod print {
    pub mod pp;
    pub mod pprust;
}

pub mod ext {
    pub mod asm;
    pub mod base;
    pub mod expand;

    pub mod quote;

    pub mod deriving;

    pub mod build;

    pub mod tt {
        pub mod transcribe;
        pub mod macro_parser;
        pub mod macro_rules;
    }


    pub mod cfg;
    pub mod fmt;
    pub mod format;
    pub mod env;
    pub mod bytes;
    pub mod concat;
    pub mod concat_idents;
    pub mod log_syntax;
    pub mod source_util;

    pub mod trace_macros;
}
