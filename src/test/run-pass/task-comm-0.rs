// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// ignore-fast

extern crate extra;

use std::task;

pub fn main() { test05(); }

fn test05_start(tx : &Sender<int>) {
    tx.send(10);
    error!("sent 10");
    tx.send(20);
    error!("sent 20");
    tx.send(30);
    error!("sent 30");
}

fn test05() {
    let (tx, rx) = channel();
    task::spawn(proc() { test05_start(&tx) });
    let mut value: int = rx.recv();
    error!("{}", value);
    value = rx.recv();
    error!("{}", value);
    value = rx.recv();
    error!("{}", value);
    assert_eq!(value, 30);
}
