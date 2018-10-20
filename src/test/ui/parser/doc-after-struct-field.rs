// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// compile-flags: -Z parse-only -Z continue-parse-after-error

struct X {
    a: u8 /** document a */,
    //~^ ERROR found a documentation comment that doesn't document anything
    //~| HELP maybe a comment was intended
}

struct Y {
    a: u8 /// document a
    //~^ ERROR found a documentation comment that doesn't document anything
    //~| HELP maybe a comment was intended
}

fn main() {
    let x = X { a: 1 };
    let y = Y { a: 1 };
}
