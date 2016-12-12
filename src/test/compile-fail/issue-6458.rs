// Copyright 2013-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::marker;

pub struct TypeWithState<State>(marker::PhantomData<State>);
pub struct MyState;

pub fn foo<State>(_: TypeWithState<State>) {}

pub fn bar() {
   foo(TypeWithState(marker::PhantomData));
   //~^ ERROR unable to infer enough type information about `State` [E0282]
   //~| NOTE cannot infer type for `State`
   //~| NOTE type annotations or generic parameter binding
}

fn main() {
}
