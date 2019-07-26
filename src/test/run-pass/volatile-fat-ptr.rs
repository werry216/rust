// run-pass

#![allow(stable_features)]
#![feature(volatile)]
use std::ptr::{read_volatile, write_volatile};

fn main() {
    let mut x: &'static str = "test";
    unsafe {
        let a = read_volatile(&x);
        assert_eq!(a, "test");
        write_volatile(&mut x, "foo");
        assert_eq!(x, "foo");
    }
}
