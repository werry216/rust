// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

trait foo {
    fn bar(&self, x: uint) -> Self;
}
impl foo for int {
    fn bar(&self) -> int {
        //~^ ERROR method `bar` has 0 parameter(s) but the trait has 1
        *self
    }
}

fn main() {
}
