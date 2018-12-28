// Tests that we consider `Box<U>: !Sugar` to be ambiguous, even
// though we see no impl of `Sugar` for `Box`. Therefore, an overlap
// error is reported for the following pair of impls (#23516).

// revisions: old re

#![cfg_attr(re, feature(re_rebalance_coherence))]

pub trait Sugar { fn dummy(&self) { } }
pub trait Sweet { fn dummy(&self) { } }
impl<T:Sugar> Sweet for T { }
impl<U:Sugar> Sweet for Box<U> { }
//[old]~^ ERROR E0119
//[re]~^^ ERROR E0119

fn main() { }
