// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn main() {
trait Seq { }

impl<T> Seq<T> for Vec<T> { //~ ERROR wrong number of type arguments
    /* ... */
}
impl Seq<bool> for u32 { //~ ERROR wrong number of type arguments
   /* Treat the integer as a sequence of bits */
}

}
