// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Issue #4036: Test for an issue that arose around fixing up type inference
// byproducts in vtable records.

extern mod extra;
use self::extra::json;
use self::extra::serialize;

pub fn main() {
    let json = json::from_str("[1]").unwrap();
    let mut decoder = json::Decoder::init(json);
    let _x: ~[int] = serialize::Decodable::decode(&mut decoder);
}
