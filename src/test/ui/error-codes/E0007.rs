#![feature(bindings_after_at)]
//~^ WARN the feature `bindings_after_at` is incomplete and may cause the compiler to crash

fn main() {
    let x = Some("s".to_string());
    match x {
        op_string @ Some(s) => {},
        //~^ ERROR E0007
        //~| ERROR E0382
        None => {},
    }
}
