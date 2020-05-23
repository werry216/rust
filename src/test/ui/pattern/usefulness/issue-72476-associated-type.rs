// check-pass

// From https://github.com/rust-lang/rust/issues/72476

trait A {
    type Projection;
}

impl A for () {
    type Projection = bool;
    // using () instead of bool here does compile though
}

struct Next<T: A>(T::Projection);

fn f(item: Next<()>) {
    match item {
        Next(true) => {}
        Next(false) => {}
    }
}

fn main() {}
