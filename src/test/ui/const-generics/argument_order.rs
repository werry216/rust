#![feature(const_generics)]
//~^ WARN the feature `const_generics` is incomplete

struct Bad<const N: usize, T> { //~ ERROR type parameters must be declared prior
    arr: [u8; { N }],
    another: T,
}

struct AlsoBad<const N: usize, 'a, T, 'b, const M: usize, U> {
    //~^ ERROR type parameters must be declared prior
    //~| ERROR lifetime parameters must be declared prior
    a: &'a T,
    b: &'b U,
}

fn main() { }
