// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(box_syntax)]

use std::thread;



fn borrow<F>(v: &isize, f: F) where F: FnOnce(&isize) {
    f(v);
}

fn box_imm() {
    let v: Box<_> = box 3;
    let w = &v;
    thread::spawn(move|| {
        println!("v={}", *v);
        //~^ ERROR cannot move `v` into closure
    });
    w.use_ref();
}

fn box_imm_explicit() {
    let v: Box<_> = box 3;
    let w = &v;
    thread::spawn(move|| {
        println!("v={}", *v);
        //~^ ERROR cannot move
    });
    w.use_ref();
}

fn main() {
}

trait Fake { fn use_mut(&mut self) { } fn use_ref(&self) { }  }
impl<T> Fake for T { }
