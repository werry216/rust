#![feature(bindings_after_at)]
//~^ WARN the feature `bindings_after_at` is incomplete and may cause the compiler to crash

fn main() {
    match Some("hi".to_string()) {
        ref op_string_ref @ Some(s) => {},
        //~^ ERROR cannot bind by-move and by-ref in the same pattern [E0009]
        None => {},
    }
}
