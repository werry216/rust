// Regression test for #66975
#![warn(const_err)]
#![feature(const_panic)]
#![feature(never_type)]

const VOID: ! = panic!();
//~^ ERROR evaluation of constant value failed

fn main() {
    let _ = VOID;
}
