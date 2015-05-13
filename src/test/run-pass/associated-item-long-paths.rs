// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::mem::size_of;

// The main point of this test is to ensure that we can parse and resolve
// associated items on associated types.

trait Foo {
    type U;
}

trait Bar {
    // Note 1: Chains of associated items in a path won't type-check.
    // Note 2: Associated consts can't depend on type parameters or `Self`,
    // which are the only types that an associated type can be referenced on for
    // now, so we can only test methods.
    fn method() -> u32;
    fn generic_method<T>() -> usize;
}

struct MyFoo;
struct MyBar;

impl Foo for MyFoo {
    type U = MyBar;
}

impl Bar for MyBar {
    fn method() -> u32 {
        2u32
    }
    fn generic_method<T>() -> usize {
        size_of::<T>()
    }
}

fn foo<T>()
    where T: Foo,
          T::U: Bar,
{
    assert_eq!(2u32, <T as Foo>::U::method());
    assert_eq!(8usize, <T as Foo>::U::generic_method::<f64>());
}

fn main() {
    foo::<MyFoo>();
}
