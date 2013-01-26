// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

struct Pair<A,B> {
    a: A, b: B
}

enum RecEnum<A> = Rec<A>;
struct Rec<A> {
    val: A,
    mut rec: Option<@RecEnum<A>>
}

fn make_cycle<A:Copy>(a: A) {
    let g: @RecEnum<A> = @RecEnum(Rec {val: a, mut rec: None});
    g.rec = Some(g);
}

fn f<A:Owned Copy, B:Owned Copy>(a: A, b: B) -> fn@() -> (A, B) {
    fn@() -> (A, B) { (a, b) }
}

fn main() {
    let x = 22_u8;
    let y = 44_u64;
    let z = f(~x, y);
    make_cycle(z);
    let (a, b) = z();
    debug!("a=%u b=%u", *a as uint, b as uint);
    assert *a == x;
    assert b == y;
}