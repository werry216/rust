// Test that const fn is illegal in a trait declaration, whether or
// not a default is provided.

#![feature(const_fn)]

trait Foo {
    const fn f() -> u32;
    //~^ ERROR functions in traits cannot be declared const
    const fn g() -> u32 {
        //~^ ERROR functions in traits cannot be declared const
        0
    }
}

fn main() {}
