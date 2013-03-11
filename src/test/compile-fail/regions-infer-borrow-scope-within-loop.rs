// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

fn borrow<T>(x: &r/T) -> &r/T {x}

fn foo(cond: &fn() -> bool, box: &fn() -> @int) {
    let mut y: &int;
    loop {
        let x = box();

	// Here we complain because the resulting region
	// of this borrow is the fn body as a whole.
        y = borrow(x); //~ ERROR illegal borrow: cannot root managed value long enough

        fail_unless!(*x == *y);
        if cond() { break; }
    }
    fail_unless!(*y != 0);
}

fn main() {}
