// Copyright 2014-2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(const_fn)]
#![allow(dead_code)]
#![warn(clippy::new_without_default, clippy::new_without_default_derive)]

pub struct Foo;

impl Foo {
    pub fn new() -> Foo {
        Foo
    }
}

pub struct Bar;

impl Bar {
    pub fn new() -> Self {
        Bar
    }
}

pub struct Ok;

impl Ok {
    pub fn new() -> Self {
        Ok
    }
}

impl Default for Ok {
    fn default() -> Self {
        Ok
    }
}

pub struct Params;

impl Params {
    pub fn new(_: u32) -> Self {
        Params
    }
}

pub struct GenericsOk<T> {
    bar: T,
}

impl<U> Default for GenericsOk<U> {
    fn default() -> Self {
        unimplemented!();
    }
}

impl<'c, V> GenericsOk<V> {
    pub fn new() -> GenericsOk<V> {
        unimplemented!()
    }
}

pub struct LtOk<'a> {
    foo: &'a bool,
}

impl<'b> Default for LtOk<'b> {
    fn default() -> Self {
        unimplemented!();
    }
}

impl<'c> LtOk<'c> {
    pub fn new() -> LtOk<'c> {
        unimplemented!()
    }
}

pub struct LtKo<'a> {
    foo: &'a bool,
}

impl<'c> LtKo<'c> {
    pub fn new() -> LtKo<'c> {
        unimplemented!()
    }
    // FIXME: that suggestion is missing lifetimes
}

struct Private;

impl Private {
    fn new() -> Private {
        unimplemented!()
    } // We don't lint private items
}

struct Const;

impl Const {
    pub const fn new() -> Const {
        Const
    } // const fns can't be implemented via Default
}

pub struct IgnoreGenericNew;

impl IgnoreGenericNew {
    pub fn new<T>() -> Self {
        IgnoreGenericNew
    } // the derived Default does not make sense here as the result depends on T
}

pub trait TraitWithNew: Sized {
    fn new() -> Self {
        panic!()
    }
}

pub struct IgnoreUnsafeNew;

impl IgnoreUnsafeNew {
    pub unsafe fn new() -> Self {
        IgnoreUnsafeNew
    }
}

#[derive(Default)]
pub struct OptionRefWrapper<'a, T: 'a>(Option<&'a T>);

impl<'a, T: 'a> OptionRefWrapper<'a, T> {
    pub fn new() -> Self {
        OptionRefWrapper(None)
    }
}

fn main() {}
