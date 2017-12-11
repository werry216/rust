// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

struct S<T> {
    t: T,
}

fn foo<T>(x: T) -> S<T> {
    S { t: x }
}

fn bar() {
    foo(4 as usize)
    //~^ ERROR mismatched types
}

fn main() {}
