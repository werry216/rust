// run-rustfix

#![allow(warnings)]

// Test that suggestion to add `*` characters applies to implementations of `Deref` as well as
// references.

struct Foo(i32);

impl std::ops::Deref for Foo {
    type Target = i32;
    fn deref(&self) -> &i32 {
        &self.0
    }
}

fn main() {
    let x = Foo(42);
    let y: i32 = x; //~ ERROR mismatched types
    let a = &42;
    let b: i32 = a; //~ ERROR mismatched types
}
