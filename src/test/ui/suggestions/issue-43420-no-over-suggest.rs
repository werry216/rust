// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// check that we substitute type parameters before we suggest anything - otherwise
// we would suggest function such as `as_slice` for the `&[u16]`.

fn foo(b: &[u16]) {}

fn main() {
    let a: Vec<u8> = Vec::new();
    foo(&a);
}
