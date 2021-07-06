// run-rustfix
#![deny(rust_2021_incompatible_closure_captures)]
//~^ NOTE: the lint level is defined here

// Test the two possible cases for automated migartion using rustfix
// - Closure contains a block i.e.  `|| { .. };`
// - Closure contains just an expr `|| ..;`

#[derive(Debug)]
struct Foo(i32);
impl Drop for Foo {
    fn drop(&mut self) {
        println!("{:?} dropped", self.0);
    }
}

fn closure_contains_block() {
    let t = (Foo(0), Foo(0));
    let c = || {
        //~^ ERROR: drop order
        //~| NOTE: for more information, see
        //~| HELP: add a dummy let to cause `t` to be fully captured
        let _t = t.0;
        //~^ NOTE: in Rust 2018, closure captures all of `t`, but in Rust 2021, it only captures `t.0`

    };

    c();
}

fn closure_doesnt_contain_block() {
    let t = (Foo(0), Foo(0));
    let c = || t.0;
    //~^ ERROR: drop order
    //~| NOTE: in Rust 2018, closure captures all of `t`, but in Rust 2021, it only captures `t.0`
    //~| NOTE: for more information, see
    //~| HELP: add a dummy let to cause `t` to be fully captured

    c();
}

fn main() {
    closure_contains_block();
    closure_doesnt_contain_block();
}
