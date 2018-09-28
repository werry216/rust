// compile-pass
// compile-flags:--cfg my_feature

#![no_std]

#[cfg(my_feature)]
extern crate std;

mod m {
    #[cfg(my_feature)]
    fn conditional() {
        std::vec::Vec::<u8>::new(); // OK
    }
}

fn main() {}
