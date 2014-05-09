// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/*!
 * C-like enums have to be represented as LLVM ints, not wrapped in a
 * struct, because it's important for the FFI that they interoperate
 * with C integers/enums, and the ABI can treat structs differently.
 * For example, on i686-linux-gnu, a struct return value is passed by
 * storing to a hidden out parameter, whereas an integer would be
 * returned in a register.
 *
 * This test just checks that the ABIs for the enum and the plain
 * integer are compatible, rather than actually calling C code.
 * The unused parameter to `foo` is to increase the likelihood of
 * crashing if something goes wrong here.
 */

#[repr(u32)]
enum Foo {
  A = 0,
  B = 23
}

#[inline(never)]
extern "C" fn foo(_x: uint) -> Foo { B }

pub fn main() {
  unsafe {
    let f: extern "C" fn(uint) -> u32 = ::std::mem::transmute(foo);
    assert_eq!(f(0xDEADBEEF), B as u32);
  }
}
