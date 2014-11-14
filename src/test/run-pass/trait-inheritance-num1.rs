// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::cmp::PartialOrd;
use std::num::NumCast;

pub trait NumExt: NumCast + PartialOrd { }

fn greater_than_one<T:NumExt>(n: &T) -> bool {
    *n > NumCast::from(1i).unwrap()
}

pub fn main() {}
