// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

enum DoubleOption<T> {
    FirstSome(T),
    AlternativeSome(T),
    Nothing,
}

fn this_function_expects_a_double_option<T>(d: DoubleOption<T>) {}

fn main() {
    let n: usize = 42;
    this_function_expects_a_double_option(n);
    //~^ ERROR mismatched types
    //~| HELP try using a variant of the expected type
}


// But don't issue the "try using a variant" help if the one-"variant" ADT is
// actually a one-field struct.

struct Payload;

struct Wrapper { payload: Payload }

struct Context { wrapper: Wrapper }

fn overton() {
    let _c = Context { wrapper: Payload{} };
    //~^ ERROR mismatched types
}
