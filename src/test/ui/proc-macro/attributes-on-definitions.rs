// check-pass
// aux-build:attributes-on-definitions.rs

#![forbid(unsafe_code)]

extern crate attributes_on_definitions;

attributes_on_definitions::with_attrs!();
//~^ WARN use of deprecated item
// No errors about the use of unstable and unsafe code inside the macro.

fn main() {}
