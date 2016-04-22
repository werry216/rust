#![feature(custom_attribute)]
#![allow(dead_code, unused_attributes)]

#[miri_run]
fn tuple() -> (i16,) {
    (1,)
}

#[miri_run]
fn tuple_2() -> (i16, i16) {
    (1, 2)
}

#[miri_run]
fn tuple_5() -> (i16, i16, i16, i16, i16) {
    (1, 2, 3, 4, 5)
}

#[derive(Debug, PartialEq)]
struct Pair { x: i8, y: i8 }

#[miri_run]
fn pair() -> Pair {
    Pair { x: 10, y: 20 }
}

#[miri_run]
fn field_access() -> (i8, i8) {
    let mut p = Pair { x: 10, y: 20 };
    p.x += 5;
    (p.x, p.y)
}

#[miri_run]
fn main() {
    assert_eq!(tuple(), (1,));
    assert_eq!(tuple_2(), (1, 2));
    assert_eq!(tuple_5(), (1, 2, 3, 4, 5));
    assert_eq!(pair(), Pair { x: 10, y: 20} );
    assert_eq!(field_access(), (15, 20));
}
