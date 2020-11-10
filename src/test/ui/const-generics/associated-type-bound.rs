// run-pass
// revisions: full min
#![cfg_attr(full, allow(incomplete_features))]
#![cfg_attr(full, feature(const_generics))]
#![cfg_attr(min, feature(min_const_generics))]

trait Bar<const N: usize> {}

trait Foo<const N: usize> {
    type Assoc: Bar<N>;
}

impl<const N: usize> Bar<N> for u8 {}
impl Bar<3> for u16 {}

impl<const N: usize> Foo<N> for i8 {
    type Assoc = u8;
}

impl Foo<3> for i16 {
    type Assoc = u16;
}

fn main() {}
