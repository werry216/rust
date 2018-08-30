// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// run-pass
use std::ops::Deref;

struct Root {
    jsref: JSRef
}

impl Deref for Root {
    type Target = JSRef;

    fn deref<'a>(&'a self) -> &'a JSRef {
        &self.jsref
    }
}

#[derive(Copy, Clone)]
struct JSRef {
    node: *const Node
}

impl Deref for JSRef {
    type Target = Node;

    fn deref<'a>(&'a self) -> &'a Node {
        self.get()
    }
}

trait INode {
    fn RemoveChild(&self);
}

impl INode for JSRef {
    fn RemoveChild(&self) {
        self.get().RemoveChild(0)
    }
}

impl JSRef {
    fn AddChild(&self) {
        self.get().AddChild(0);
    }

    fn get<'a>(&'a self) -> &'a Node {
        unsafe {
            &*self.node
        }
    }
}

struct Node;

impl Node {
    fn RemoveChild(&self, _a: usize) {
    }

    fn AddChild(&self, _a: usize) {
    }
}

fn main() {
    let n = Node;
    let jsref = JSRef { node: &n };
    let root = Root { jsref: jsref };

    root.AddChild();
    jsref.AddChild();

    root.RemoveChild();
    jsref.RemoveChild();
}
