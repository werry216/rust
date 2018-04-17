// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test where we might in theory be able to see that the relationship
// between two bound regions is true within closure and hence have no
// need to propagate; but in fact we do because identity of free
// regions is erased.

// compile-flags:-Zborrowck=mir -Zverbose
// compile-pass

#![feature(rustc_attrs)]

use std::cell::Cell;

// In theory, callee knows that:
//
// 'x: 'a
// 'a: 'y
//
// and hence could satisfy that `'x: 'y` locally. However, in our
// checking, we ignore the precise free regions that come into the
// region and just assign each position a distinct universally bound
// region. Hence, we propagate a constraint to our caller that will
// wind up being solvable.
fn establish_relationships<'a, F>(
    _cell_a: Cell<&'a u32>,
    _closure: F,
) where
    F: for<'x, 'y> FnMut(
        Cell<&'a &'x u32>, // shows that 'x: 'a
        Cell<&'y &'a u32>, // shows that 'a: 'y
        Cell<&'x u32>,
        Cell<&'y u32>,
    ),
{
}

fn demand_y<'x, 'y>(_cell_x: Cell<&'x u32>, _cell_y: Cell<&'y u32>, _y: &'y u32) {}

#[rustc_regions]
fn supply<'a>(cell_a: Cell<&'a u32>) {
    establish_relationships(
        cell_a,
        |_outlives1, _outlives2, x, y| {
            // Only works if 'x: 'y:
            let p = x.get();
            demand_y(x, y, p)
        },
    );
}

fn main() {}
