// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// xfail-fast
// aux-build:trait_inheritance_auto_xc_aux.rs

extern mod aux(name = "trait_inheritance_auto_xc_aux");

use aux::{Foo, Bar, Baz, Quux};

struct A { x: int }

impl Foo for A { fn f() -> int { 10 } }
impl Bar for A { fn g() -> int { 20 } }
impl Baz for A { fn h() -> int { 30 } }

fn f<T:Quux>(a: &T) {
    fail_unless!(a.f() == 10);
    fail_unless!(a.g() == 20);
    fail_unless!(a.h() == 30);
}

pub fn main() {
    let a = &A { x: 3 };
    f(a);
}

