// aux-build:on_structs_and_enums_xc.rs

extern crate on_structs_and_enums_xc;

use on_structs_and_enums_xc::{Bar, Foo, Trait};

fn main() {
    let foo = Foo {
    //~^ ERROR E0277
        x: 3
    };
    let bar: Bar<f64> = return;
    //~^ ERROR E0277
    let _ = bar;
}
