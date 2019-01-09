// run-pass
#![allow(dead_code)]
#![allow(unused_variables)]
#![allow(non_camel_case_types)]


// check that the &isize here does not cause us to think that `foo`
// contains region pointers
// pretty-expanded FIXME #23616

struct foo(Box<FnMut(&isize)+'static>);

fn take_foo<T:'static>(x: T) {}

fn have_foo(f: foo) {
    take_foo(f);
}

fn main() {}
