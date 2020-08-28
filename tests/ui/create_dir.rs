// run-rustfix
#![allow(unused_must_use)]
#![warn(clippy::create_dir)]

fn not_create_dir() {}

fn main() {
    std::fs::create_dir("foo");
    std::fs::create_dir("bar").unwrap();

    not_create_dir();
    std::fs::create_dir_all("foobar");
}
