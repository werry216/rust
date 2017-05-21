// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Test inclusive range syntax.

#![feature(inclusive_range_syntax, inclusive_range, step_by)]

use std::ops::{RangeInclusive, RangeToInclusive};

fn foo() -> isize { 42 }

// Test that range syntax works in return statements
fn return_range_to() -> RangeToInclusive<i32> { return ...1; }

pub fn main() {
    let mut count = 0;
    for i in 0_usize...10 {
        assert!(i >= 0 && i <= 10);
        count += i;
    }
    assert_eq!(count, 55);

    let mut count = 0;
    let mut range = 0_usize...10;
    for i in range {
        assert!(i >= 0 && i <= 10);
        count += i;
    }
    assert_eq!(count, 55);

    let mut count = 0;
    for i in (0_usize...10).step_by(2) {
        assert!(i >= 0 && i <= 10 && i % 2 == 0);
        count += i;
    }
    assert_eq!(count, 30);

    let _ = 0_usize...4+4-3;
    let _ = 0...foo();

    let _ = { &42...&100 }; // references to literals are OK
    let _ = ...42_usize;

    // Test we can use two different types with a common supertype.
    let x = &42;
    {
        let y = 42;
        let _ = x...&y;
    }

    // test collection indexing
    let vec = (0...10).collect::<Vec<_>>();
    let slice: &[_] = &*vec;
    let string = String::from("hello world");
    let stir = "hello world";

    assert_eq!(&vec[3...6], &[3, 4, 5, 6]);
    assert_eq!(&vec[ ...6], &[0, 1, 2, 3, 4, 5, 6]);

    assert_eq!(&slice[3...6], &[3, 4, 5, 6]);
    assert_eq!(&slice[ ...6], &[0, 1, 2, 3, 4, 5, 6]);

    assert_eq!(&string[3...6], "lo w");
    assert_eq!(&string[ ...6], "hello w");

    assert_eq!(&stir[3...6], "lo w");
    assert_eq!(&stir[ ...6], "hello w");

    // test the size hints and emptying
    let mut long = 0...255u8;
    let mut short = 42...42u8;
    assert_eq!(long.size_hint(), (256, Some(256)));
    assert_eq!(short.size_hint(), (1, Some(1)));
    long.next();
    short.next();
    assert_eq!(long.size_hint(), (255, Some(255)));
    assert_eq!(short.size_hint(), (0, Some(0)));
    assert_eq!(short, 1...0);

    assert_eq!(long.len(), 255);
    assert_eq!(short.len(), 0);

    // test iterating backwards
    assert_eq!(long.next_back(), Some(255));
    assert_eq!(long.next_back(), Some(254));
    assert_eq!(long.next_back(), Some(253));
    assert_eq!(long.next(), Some(1));
    assert_eq!(long.next(), Some(2));
    assert_eq!(long.next_back(), Some(252));
    for i in 3...251 {
        assert_eq!(long.next(), Some(i));
    }
    assert_eq!(long, 1...0);

    // check underflow
    let mut narrow = 1...0;
    assert_eq!(narrow.next_back(), None);
    assert_eq!(narrow, 1...0);
    let mut zero = 0u8...0;
    assert_eq!(zero.next_back(), Some(0));
    assert_eq!(zero.next_back(), None);
    assert_eq!(zero, 1...0);
    let mut high = 255u8...255;
    assert_eq!(high.next_back(), Some(255));
    assert_eq!(high.next_back(), None);
    assert_eq!(high, 1...0);

    // what happens if you have a nonsense range?
    let mut nonsense = 10...5;
    assert_eq!(nonsense.next(), None);
    assert_eq!(nonsense, 10...5);

    // output
    assert_eq!(format!("{:?}", 0...10), "0...10");
    assert_eq!(format!("{:?}", ...10), "...10");
    assert_eq!(format!("{:?}", long), "1...0");
}
