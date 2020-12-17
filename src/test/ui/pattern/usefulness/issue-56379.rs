enum Foo {
    A(bool),
    B(bool),
    C(bool),
}

fn main() {
    match Foo::A(true) {
        //~^ ERROR non-exhaustive patterns: `A(false)` not covered
        Foo::A(true) => {}
        Foo::B(true) => {}
        Foo::C(true) => {}
    }
}
