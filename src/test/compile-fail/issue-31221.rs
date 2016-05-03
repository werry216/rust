// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

enum Enum {
    Var1,
    Var2,
}

fn main() {
    use Enum::*;
    let s = Var1;
    match s {
        Var1 => (),
        Var3 => (),
        //~^ NOTE this pattern matches any value
        Var2 => (),
        //~^ ERROR unreachable pattern
    };
    match &s {
        &Var1 => (),
        &Var3 => (),
        //~^ NOTE this pattern matches any value
        &Var2 => (),
        //~^ ERROR unreachable pattern
    };
    let t = (Var1, Var1);
    match t {
        (Var1, b) => (),
        (c, d) => (),
        //~^ NOTE this pattern matches any value
        anything => ()
        //~^ ERROR unreachable pattern
    };
    // `_` need not emit a note, it is pretty obvious already.
    let t = (Var1, Var1);
    match t {
        (Var1, b) => (),
        _ => (),
        anything => ()
        //~^ ERROR unreachable pattern
    };
}
