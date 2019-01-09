// run-pass
//compile-flags: -Z borrowck=compare

#![deny(warnings)]

fn foo<F: FnOnce()>(_f: F) { }

fn main() {
    let mut var = Vec::new();
    foo(move|| {
        var.push(1);
    });
}
