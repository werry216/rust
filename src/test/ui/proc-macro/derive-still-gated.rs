// aux-build:test-macros.rs

#[macro_use]
extern crate test_macros;

#[derive_Empty] //~ ERROR cannot find attribute macro `derive_Empty` in this scope
struct A;

fn main() {}
