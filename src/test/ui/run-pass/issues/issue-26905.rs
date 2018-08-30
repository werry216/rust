// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// run-pass
#![feature(unsize, coerce_unsized)]

// Verfies that PhantomData is ignored for DST coercions

use std::marker::{Unsize, PhantomData};
use std::ops::CoerceUnsized;

struct MyRc<T: ?Sized> {
    _ptr: *const T,
    _boo: PhantomData<T>,
}

impl<T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<MyRc<U>> for MyRc<T>{ }

fn main() {
    let data = [1, 2, 3];
    let iter = data.iter();
    let x = MyRc { _ptr: &iter, _boo: PhantomData };
    let _y: MyRc<Iterator<Item=&u32>> = x;
}

