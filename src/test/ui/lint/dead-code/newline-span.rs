#![deny(dead_code)]

fn unused() { //~ error: function is never used:
    println!("blah");
}

fn unused2(var: i32) { //~ error: function is never used:
    println!("foo {}", var);
}

fn unused3( //~ error: function is never used:
    var: i32,
) {
    println!("bar {}", var);
}

fn main() {
    println!("Hello world!");
}
