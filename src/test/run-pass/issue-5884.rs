// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

pub struct Foo {
    a: int,
}

struct Bar<'self> {
    a: ~Option<int>,
    b: &'self Foo,
}

fn check(a: @Foo) {
    let mut _ic = Bar{ b: a, a: ~None };
}

pub fn main(){}
