// run-rustfix
// compile-flags: --edition 2018
#![warn(clippy::single_component_path_imports)]
#![allow(unused_imports)]

use regex;
use serde as edres;
pub use serde;

fn main() {
    regex::Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
}
