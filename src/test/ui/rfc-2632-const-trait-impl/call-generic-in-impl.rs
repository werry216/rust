// check-pass
#![feature(const_fn_trait_bound)]
#![feature(const_trait_impl)]

trait MyPartialEq {
    fn eq(&self, other: &Self) -> bool;
}

impl<T: PartialEq> const MyPartialEq for T {
    fn eq(&self, other: &Self) -> bool {
        PartialEq::eq(self, other)
    }
}

fn main() {}
