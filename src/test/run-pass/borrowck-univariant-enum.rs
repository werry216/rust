// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

enum newtype {
    newtype(int)
}

pub fn main() {

    // Test that borrowck treats enums with a single variant
    // specially.

    let x = @mut 5;
    let y = @mut newtype(3);
    let z = match *y {
      newtype(b) => {
        *x += 1;
        *x * b
      }
    };
    fail_unless!(z == 18);
}
