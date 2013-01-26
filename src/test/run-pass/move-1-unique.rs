// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

struct Triple { x: int, y: int, z: int }

fn test(x: bool, foo: ~Triple) -> int {
    let bar = foo;
    let mut y: ~Triple;
    if x { y = move bar; } else { y = ~Triple{x: 4, y: 5, z: 6}; }
    return y.y;
}

fn main() {
    let x = ~Triple{x: 1, y: 2, z: 3};
    assert (test(true, x) == 2);
    assert (test(true, x) == 2);
    assert (test(true, x) == 2);
    assert (test(false, x) == 5);
}
