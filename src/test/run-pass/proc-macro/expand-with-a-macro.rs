// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:expand-with-a-macro.rs

// ignore-wasm32-bare compiled with panic=abort by default

#![deny(warnings)]

#[macro_use]
extern crate expand_with_a_macro;

use std::panic;

#[derive(A)]
struct A;

fn main() {
    assert!(panic::catch_unwind(|| {
        A.a();
    }).is_err());
}

