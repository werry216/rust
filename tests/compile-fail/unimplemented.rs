#![feature(custom_attribute)]
#![allow(dead_code, unused_attributes)]

//error-pattern:begin_panic_fmt


#[miri_run]
fn failed_assertions() {
    assert_eq!(5, 6);
}

fn main() {}
