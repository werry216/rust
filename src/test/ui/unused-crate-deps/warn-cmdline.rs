// Check for unused crate dep, no path

// edition:2018
// check-pass
// compile-flags: -Wunused-crate-dependencies
// aux-crate:bar=bar.rs

fn main() {}
//~^ WARNING external crate `bar` unused in
