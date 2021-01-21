// run-rustfix

#![deny(clippy::exhaustive_enums)]
#![allow(unused)]

fn main() {
    // nop
}

pub enum Exhaustive {
    Foo,
    Bar,
    Baz,
    Quux(String),
}

// no warning, already non_exhaustive
#[non_exhaustive]
pub enum NonExhaustive {
    Foo,
    Bar,
    Baz,
    Quux(String),
}

// no warning, private
enum ExhaustivePrivate {
    Foo,
    Bar,
    Baz,
    Quux(String),
}

// no warning, private
#[non_exhaustive]
enum NonExhaustivePrivate {
    Foo,
    Bar,
    Baz,
    Quux(String),
}
