// run-rustfix

#![warn(clippy::match_wildcard_for_single_variants)]
#![allow(dead_code)]

enum Foo {
    A,
    B,
    C,
}

fn main() {
    match Foo::A {
        Foo::A => {},
        Foo::B => {},
        _ => {},
    }
}
