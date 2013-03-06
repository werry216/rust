// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.



fn box<T:Copy>(x: Box<T>) -> @Box<T> { return @x; }

struct Box<T> {x: T, y: T, z: T}

pub fn main() {
    let x: @Box<int> = box::<int>(Box{x: 1, y: 2, z: 3});
    fail_unless!((x.y == 2));
}
