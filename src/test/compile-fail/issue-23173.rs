// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

enum Token { LeftParen, RightParen, Plus, Minus, /* etc */ }
//~^ NOTE variant `Homura` not found here

fn use_token(token: &Token) { unimplemented!() }

fn main() {
    use_token(&Token::Homura);
    //~^ ERROR no variant named `Homura`
    //~| NOTE variant not found in `Token`
}
