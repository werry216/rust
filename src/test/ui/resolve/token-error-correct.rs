// Test that we do some basic error correction in the tokeniser.

fn main() {
    foo(bar(;
    //~^ ERROR cannot find function `bar` in this scope
}
//~^ ERROR: mismatched closing delimiter: `}`

fn foo(_: usize) {}
