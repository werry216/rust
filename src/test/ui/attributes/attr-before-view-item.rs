// compile-pass
// pretty-expanded FIXME #23616

#![feature(rustc_attrs)]
#![feature(test)]

#[rustc_dummy = "bar"]
extern crate test;

fn main() {}
