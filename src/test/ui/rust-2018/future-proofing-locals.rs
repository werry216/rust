// edition:2018

#![feature(uniform_paths)]

mod T {
    pub struct U;
}
mod x {
    pub struct y;
}

fn type_param<T>() {
    use T as _; //~ ERROR imports cannot refer to type parameters
    use T::U; //~ ERROR imports cannot refer to type parameters
    use T::*; //~ ERROR imports cannot refer to type parameters
}

fn self_import<T>() {
    use T; // FIXME Should be an error, but future-proofing fails due to `T` being "self-shadowed"
}

fn let_binding() {
    let x = 10;

    use x as _; //~ ERROR imports cannot refer to local variables
    use x::y; // OK
    use x::*; // OK
}

fn param_binding(x: u8) {
    use x; //~ ERROR imports cannot refer to local variables
}

fn match_binding() {
    match 0 {
        x => {
            use x; //~ ERROR imports cannot refer to local variables
        }
    }
}

fn nested<T>() {
    let x = 10;

    use {T as _, x}; //~ ERROR imports cannot refer to type parameters
                     //~| ERROR imports cannot refer to local variables
}

fn main() {}
