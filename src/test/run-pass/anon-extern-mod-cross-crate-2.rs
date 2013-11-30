// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// xfail-fast
// xfail-pretty
// aux-build:anon-extern-mod-cross-crate-1.rs
extern mod anonexternmod;

use anonexternmod::rust_get_test_int;

#[link(name = "rustrt")] // we have explicitly chosen to require this
extern {}

pub fn main() {
    unsafe {
        rust_get_test_int();
    }
}
