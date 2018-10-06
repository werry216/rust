// Copyright 2014-2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


//ignore-x86_64



#[warn(wrong_transmute)]
fn main() {
    unsafe {
        let _: *const usize = std::mem::transmute(6.0f32);

        let _: *mut usize = std::mem::transmute(6.0f32);

        let _: *const usize = std::mem::transmute('x');

        let _: *mut usize = std::mem::transmute('x');
    }
}
