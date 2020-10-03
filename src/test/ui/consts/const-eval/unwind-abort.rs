#![feature(unwind_attributes, const_panic)]

#[unwind(aborts)]
const fn foo() {
    panic!() //~ evaluation of constant value failed
}

const _: () = foo(); //~ any use of this value will cause an error

fn main() {
    let _ = foo();
}
