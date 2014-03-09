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

struct complainer {
    tx: Sender<bool>,
}

impl Drop for complainer {
    fn drop(&mut self) {
        error!("About to send!");
        self.tx.send(true);
        error!("Sent!");
    }
}

fn complainer(tx: Sender<bool>) -> complainer {
    error!("Hello!");
    complainer {
        tx: tx
    }
}

fn f(tx: Sender<bool>) {
    let _tx = complainer(tx);
    fail!();
}

pub fn main() {
    let (tx, rx) = channel();
    task::spawn(proc() f(tx.clone()));
    error!("hiiiiiiiii");
    assert!(rx.recv());
}
