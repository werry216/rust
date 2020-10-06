// Traits with bounds mentioning `Self` are not object safe

trait X {
    type U: PartialEq<Self>;
}

fn f() -> Box<dyn X<U = u32>> {
    //~^ ERROR the trait `X` cannot be made into an object
    loop {}
}

fn main() {}
