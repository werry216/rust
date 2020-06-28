#![feature(const_generics)]
#![allow(incomplete_features)]

struct Test();

fn pass() {
    println!("Hello, world!");
}

impl Test {
    pub fn call_me(&self) {
        self.test::<pass>();
    }

    fn test<const FN: fn()>(&self) {
        //~^ ERROR: using function pointers as const generic parameters is forbidden
        FN();
    }
}

fn main() {
    let x = Test();
    x.call_me()
}
