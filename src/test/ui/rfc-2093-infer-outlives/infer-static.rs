// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(rustc_attrs)]
#![feature(infer_outlives_requirements)]
#![feature(infer_static_outlives_requirements)]

#[rustc_outlives]
struct Foo<U> { //~ ERROR 16:1: 18:2: rustc_outlives
    bar: Bar<U>
}
struct Bar<T: 'static> {
    x: T,
}

fn main() {}

