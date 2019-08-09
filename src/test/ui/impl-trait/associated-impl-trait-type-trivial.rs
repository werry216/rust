#![feature(type_alias_impl_trait)]
// build-pass (FIXME(62277): could be check-pass?)

trait Bar {}
struct Dummy;
impl Bar for Dummy {}

trait Foo {
    type Assoc: Bar;
    fn foo() -> Self::Assoc;
}

impl Foo for i32 {
    type Assoc = impl Bar;
    fn foo() -> Self::Assoc {
        Dummy
    }
}

fn main() {}
