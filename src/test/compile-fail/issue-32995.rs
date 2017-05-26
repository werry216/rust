// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![deny(parenthesized_params_in_types_and_modules)]
//~^ NOTE lint level defined here
//~| NOTE lint level defined here
//~| NOTE lint level defined here
//~| NOTE lint level defined here
//~| NOTE lint level defined here
//~| NOTE lint level defined here
//~| NOTE lint level defined here
#![allow(dead_code, unused_variables)]

fn main() {
    let x: usize() = 1;
    //~^ ERROR parenthesized parameters may only be used with a trait
    //~| WARN previously accepted
    //~| NOTE issue #42238

    let b: ::std::boxed()::Box<_> = Box::new(1);
    //~^ ERROR parenthesized parameters may only be used with a trait
    //~| WARN previously accepted
    //~| NOTE issue #42238

    macro_rules! pathexpr {
        ($p:path) => { $p }
    }

    let p = pathexpr!(::std::str()::from_utf8)(b"foo").unwrap();
    //~^ ERROR parenthesized parameters may only be used with a trait
    //~| WARN previously accepted
    //~| NOTE issue #42238

    let p = pathexpr!(::std::str::from_utf8())(b"foo").unwrap();
    //~^ ERROR parenthesized parameters may only be used with a trait
    //~| WARN previously accepted
    //~| NOTE issue #42238

    let o : Box<::std::marker()::Send> = Box::new(1);
    //~^ ERROR parenthesized parameters may only be used with a trait
    //~| WARN previously accepted
    //~| NOTE issue #42238

    let o : Box<Send + ::std::marker()::Sync> = Box::new(1);
    //~^ ERROR parenthesized parameters may only be used with a trait
    //~| WARN previously accepted
    //~| NOTE issue #42238
}

fn foo<X:Default>() {
    let d : X() = Default::default();
    //~^ ERROR parenthesized parameters may only be used with a trait
    //~| WARN previously accepted
    //~| NOTE issue #42238
}
