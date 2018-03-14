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

fn foo(x: !, y: usize) { }

fn bar(x: !) { }

fn a() {
    // the `22` is unreachable:
    foo(return, 22); //~ ERROR unreachable
}

fn b() {
    // the call is unreachable:
    bar(return); //~ ERROR unreachable
}

fn main() { }
