// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![allow(unused_variables)]
#![allow(unused_assignments)]
#![allow(dead_code)]
#![deny(unreachable_code)]
#![feature(never_type)]

fn a() {
    // Here the tail expression is considered unreachable:
    let x = {
        return;
        22
    };
}

fn b() {
    // Here the `x` assignment is considered unreachable, not the block:
    let x = {
        return;
    };
}

fn c() {
    // Here the `println!` is unreachable:
    let x = {
        return;
        println!("foo");
        22
    };
}

fn main() { }
