#![feature(plugin, collections)]
#![feature(associated_type_defaults)]
#![feature(associated_consts)]

#![plugin(clippy)]
#![deny(clippy)]
#![allow(dead_code, needless_take_by_value)]

extern crate collections;
use collections::linked_list::LinkedList;

trait Foo {
    type Baz = LinkedList<u8>;
    fn foo(LinkedList<u8>);
    const BAR : Option<LinkedList<u8>>;
}

// ok, we don’t want to warn for implementations, see #605
impl Foo for LinkedList<u8> {
    fn foo(_: LinkedList<u8>) {}
    const BAR : Option<LinkedList<u8>> = None;
}

struct Bar;
impl Bar {
    fn foo(_: LinkedList<u8>) {}
}

pub fn test(my_favourite_linked_list: LinkedList<u8>) {
    println!("{:?}", my_favourite_linked_list)
}

pub fn test_ret() -> Option<LinkedList<u8>> {
    unimplemented!();
}

fn main(){
    test(LinkedList::new());
}
