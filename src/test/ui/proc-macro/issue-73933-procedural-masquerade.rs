// check-pass
// aux-build:test-macros.rs

#[macro_use]
extern crate test_macros;

#[derive(Print)]
enum ProceduralMasqueradeDummyType { //~ WARN using
//~| WARN this was previously
    Input
}

fn main() {}
