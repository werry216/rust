// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// compile-flags: -Zborrowck=mir

#![allow(dead_code)]

// Test that we relate the type of the fn type to the type of the fn
// ptr when doing a `ReifyFnPointer` cast.
//
// This test is a bit tortured, let me explain:
//

// The `where 'a: 'a` clause here ensures that `'a` is early bound,
// which is needed below to ensure that this test hits the path we are
// concerned with.
fn foo<'a>(x: &'a u32) -> &'a u32
where
    'a: 'a,
{
    panic!()
}

fn bar<'a>(x: &'a u32) -> &'static u32 {
    // Here, the type of `foo` is `typeof(foo::<'x>)` for some fresh variable `'x`.
    // During NLL region analysis, this will get renumbered to `typeof(foo::<'?0>)`
    // where `'?0` is a new region variable.
    //
    // (Note that if `'a` on `foo` were late-bound, the type would be
    // `typeof(foo)`, which would interact differently with because
    // the renumbering later.)
    //
    // This type is then coerced to a fn type `fn(&'?1 u32) -> &'?2
    // u32`. Here, the `'?1` and `'?2` will have been created during
    // the NLL region renumbering.
    //
    // The MIR type checker must therefore relate `'?0` to `'?1` and `'?2`
    // as part of checking the `ReifyFnPointer`.
    let f: fn(_) -> _ = foo;
    f(x)
    //~^ ERROR unsatisfied lifetime constraints
}

fn main() {}
