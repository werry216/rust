// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// run-pass
#![allow(unused_must_use)]
#![feature(decl_macro)]

pub macro create_struct($a:ident) {
    struct $a;
    impl Clone for $a {
        fn clone(&self) -> Self {
            $a
        }
    }
}

fn main() {
    create_struct!(Test);
    Test.clone();
}
