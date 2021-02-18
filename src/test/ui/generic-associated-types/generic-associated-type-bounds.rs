// run-pass

#![allow(incomplete_features)]
#![feature(generic_associated_types)]

pub trait X {
    type Y<'a>;
    fn m(&self) -> Self::Y<'_>;
}

impl X for () {
    type Y<'a> = &'a ();

    fn m(&self) -> Self::Y<'_> {
        self
    }
}

fn f(x: &impl for<'a> X<Y<'a> = &'a ()>) -> &() {
    x.m()
}

fn g<T: for<'a> X<Y<'a> = &'a ()>>(x: &T) -> &() {
    x.m()
}

fn h(x: &()) -> &() {
    x.m()
}

fn main() {
    f(&());
    g(&());
    h(&());
}
