// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// run-pass
// aux-build:issue-29485.rs
// ignore-emscripten no threads

#[feature(recover)]

extern crate a;

fn main() {
    let _ = std::thread::spawn(move || {
        a::f(&mut a::X(0), g);
    }).join();
}

fn g() {
    panic!();
}
