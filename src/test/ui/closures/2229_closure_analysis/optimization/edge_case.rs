#![feature(capture_disjoint_fields)]
//~^ WARNING: the feature `capture_disjoint_fields` is incomplete
//~| NOTE: `#[warn(incomplete_features)]` on by default
//~| NOTE: see issue #53488 <https://github.com/rust-lang/rust/issues/53488>

#![feature(rustc_attrs)]
#![allow(unused)]
#![allow(dead_code)]

struct Int(i32);
struct B<'a>(&'a i32);

const I : Int = Int(0);
const REF_I : &'static Int = &I;


struct MyStruct<'a> {
   a: &'static Int,
   b: B<'a>,
}

fn foo<'a, 'b>(m: &'a MyStruct<'b>) -> impl FnMut() + 'static {
    let c = #[rustc_capture_analysis] || drop(&m.a.0);
    //~^ ERROR: attributes on expressions are experimental
    //~| NOTE: see issue #15701 <https://github.com/rust-lang/rust/issues/15701>
    //~| ERROR: First Pass analysis includes:
    //~| ERROR: Min Capture analysis includes:
    //~| NOTE: Capturing m[Deref,(0, 0),Deref] -> ImmBorrow
    //~| NOTE: Min Capture m[Deref,(0, 0),Deref] -> ImmBorrow
    c
}

fn main() {
    let t = 0;
    let s = MyStruct { a: REF_I, b: B(&t) };
    let _ = foo(&s);
}
