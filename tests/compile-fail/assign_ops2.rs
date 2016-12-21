#![feature(plugin)]
#![plugin(clippy)]

#[allow(unused_assignments)]
#[deny(misrefactored_assign_op)]
fn main() {
    let mut a = 5;
    a += a + 1; //~ ERROR variable appears on both sides of an assignment operation
    //~^ HELP replace it with
    //~| SUGGESTION a += 1
    a += 1 + a; //~ ERROR variable appears on both sides of an assignment operation
    //~^ HELP replace it with
    //~| SUGGESTION a += 1
    a -= a - 1; //~ ERROR variable appears on both sides of an assignment operation
    //~^ HELP replace it with
    //~| SUGGESTION a -= 1
    a *= a * 99; //~ ERROR variable appears on both sides of an assignment operation
    //~^ HELP replace it with
    //~| SUGGESTION a *= 99
    a *= 42 * a; //~ ERROR variable appears on both sides of an assignment operation
    //~^ HELP replace it with
    //~| SUGGESTION a *= 42
    a /= a / 2; //~ ERROR variable appears on both sides of an assignment operation
    //~^ HELP replace it with
    //~| SUGGESTION a /= 2
    a %= a % 5; //~ ERROR variable appears on both sides of an assignment operation
    //~^ HELP replace it with
    //~| SUGGESTION a %= 5
    a &= a & 1; //~ ERROR variable appears on both sides of an assignment operation
    //~^ HELP replace it with
    //~| SUGGESTION a &= 1
    a -= 1 - a;
    a /= 5 / a;
    a %= 42 % a;
    a <<= 6 << a;
}

// check that we don't lint on op assign impls, because that's just the way to impl them

use std::ops::{Mul, MulAssign};

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Wrap(i64);

impl Mul<i64> for Wrap {
    type Output = Self;

    fn mul(self, rhs: i64) -> Self {
        Wrap(self.0 * rhs)
    }
}

impl MulAssign<i64> for Wrap {
    fn mul_assign(&mut self, rhs: i64) {
        *self = *self * rhs
    }
}
