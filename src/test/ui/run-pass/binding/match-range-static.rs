// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// run-pass
// pretty-expanded FIXME #23616
#![allow(non_upper_case_globals)]

const s: isize = 1;
const e: isize = 42;

pub fn main() {
    match 7 {
        s..=e => (),
        _ => (),
    }
}
