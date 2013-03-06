// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.




// -*- rust -*-
struct Point {x: int, y: int, z: int}

fn f(p: &mut Point) { p.z = 13; }

pub fn main() {
    let mut x: Point = Point {x: 10, y: 11, z: 12};
    f(&mut x);
    fail_unless!((x.z == 13));
}
