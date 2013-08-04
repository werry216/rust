// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn sum_slice(x: &[int]) -> int {
    let mut sum = 0;
    for i in x.iter() { sum += *i; }
    return sum;
}

pub fn main() {
    let x = @[1, 2, 3];
    assert_eq!(sum_slice(x), 6);
}
