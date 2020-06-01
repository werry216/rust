// compile-flags: -Zunleash-the-miri-inside-of-you
#![feature(thread_local)]
#![allow(const_err)]

use std::thread;

#[thread_local]
static A: u8 = 0;

// Make sure we catch executing inline assembly.
static TEST_BAD: () = {
    unsafe { let _val = A; }
    //~^ ERROR could not evaluate static initializer
    //~| NOTE cannot access thread local static
};

fn main() {}
