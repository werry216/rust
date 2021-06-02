#![crate_name = "foo"]
#![feature(const_generics)]

pub trait Array {
    type Item;
}

// @has foo/trait.Array.html
// @has - '//div[@class="impl has-srclink"]' 'impl<T, const N: usize> Array for [T; N]'
impl <T, const N: usize> Array for [T; N] {
    type Item = T;
}
