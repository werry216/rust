#![feature(plugin)]

#![plugin(clippy)]
#![deny(neg_multiply)]
#![allow(no_effect)]

use std::ops::Mul;

struct X;

impl Mul<isize> for X {
    type Output = X;
    
    fn mul(self, _r: isize) -> Self {
        self
    }
}

impl Mul<X> for isize {
    type Output = X;
    
    fn mul(self, _r: X) -> X {
        X
    }
}

fn main() {
    let x = 0;

    x * -1;
    //~^ ERROR Negation by multiplying with -1

    -1 * x;
    //~^ ERROR Negation by multiplying with -1

    -1 * -1; // should be ok
    
    X * -1; // should be ok
    -1 * X; // should also be ok
}
