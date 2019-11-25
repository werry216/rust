// issue 65419 - Attempting to run an `async fn` after completion mentions generators when it should
// be talking about `async fn`s instead. Regression test added to make sure generators still
// panic when resumed after completion.

// run-fail
// error-pattern:generator resumed after completion

#![feature(generators, generator_trait)]

use std::{
    ops::Generator,
    pin::Pin,
};

fn main() {
    let mut g = || {
        yield;
    };
    Pin::new(&mut g).resume(); // Yields once.
    Pin::new(&mut g).resume(); // Completes here.
    Pin::new(&mut g).resume(); // Panics here.
}
