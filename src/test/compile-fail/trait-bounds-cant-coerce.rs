// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


trait Foo {
}

fn a(_x: Box<Foo+Send>) {
}

fn c(x: Box<Foo+Share+Send>) {
    a(x);
}

fn d(x: Box<Foo>) {
    a(x); //~ ERROR found no bounds
}

fn main() { }
