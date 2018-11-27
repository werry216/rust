// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(nll)]

use std::rc::Rc;
use std::sync::Arc;

struct Bar { field: Vec<i32> }

fn main() {
    let x = Rc::new(Bar { field: vec![] });
    drop(x.field);
//~^ ERROR cannot move out of an `Rc`

    let y = Arc::new(Bar { field: vec![] });
    drop(y.field);
//~^ ERROR cannot move out of an `Arc`
}
