#![feature(plugin)]
#![plugin(clippy)]
#![deny(modulo_one)]
#![allow(no_effect, unnecessary_operation)]

fn main() {
    10 % 1;
    10 % 2;
}
