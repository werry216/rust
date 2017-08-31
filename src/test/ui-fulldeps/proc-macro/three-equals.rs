// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// aux-build:three-equals.rs
// ignore-stage1

#![feature(proc_macro)]

extern crate three_equals;

use three_equals::three_equals;

fn main() {
    // This one is okay.
    three_equals!(===);

    // Need exactly three equals.
    three_equals!(==);

    // Need exactly three equals.
    three_equals!(=====);

    // Only equals accepted.
    three_equals!(abc);

    // Only equals accepted.
    three_equals!(!!);

    // Only three characters expected.
    three_equals!(===a);
}
