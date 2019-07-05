// check-pass
// edition:2018
// aux-build:test-macros.rs

// Anonymize unstable non-dummy spans while still showing dummy spans `0..0`.
// normalize-stdout-test "bytes\([^0]\w*\.\.(\w+)\)" -> "bytes(LO..$1)"
// normalize-stdout-test "bytes\((\w+)\.\.[^0]\w*\)" -> "bytes($1..HI)"

#![feature(proc_macro_hygiene)]

#[macro_use]
extern crate test_macros;

type S = u8;

macro_rules! m { () => {
    #[print_attr]
    struct A(identity!($crate::S));
}}

m!();

fn main() {}
