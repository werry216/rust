#![allow(unused)]
//#![feature(plugin)]#![plugin(clippy)]
use std::collections::{HashMap, HashSet};
use std::cmp::Eq;
use std::hash::{Hash, BuildHasher};

trait Foo<T>: Sized {
    fn make() -> (Self, Self);
}

impl<K: Hash + Eq, V> Foo<i8> for HashMap<K, V> {
    fn make() -> (Self, Self) {
        // OK, don't suggest to modify these
        let _: HashMap<i32, i32> = HashMap::new();
        let _: HashSet<i32> = HashSet::new();

        (HashMap::new(), HashMap::with_capacity(10))
    }
}
impl<K: Hash + Eq, V> Foo<i8> for (HashMap<K, V>,) {
    fn make() -> (Self, Self) {
        ((HashMap::new(),), (HashMap::with_capacity(10),))
    }
}
impl Foo<i16> for HashMap<String, String> {
    fn make() -> (Self, Self) {
        (HashMap::new(), HashMap::with_capacity(10))
    }
}

impl<K: Hash + Eq, V, S: BuildHasher + Default> Foo<i32> for HashMap<K, V, S> {
    fn make() -> (Self, Self) {
        (HashMap::default(), HashMap::with_capacity_and_hasher(10, Default::default()))
    }
}
impl<S: BuildHasher + Default> Foo<i64> for HashMap<String, String, S> {
    fn make() -> (Self, Self) {
        (HashMap::default(), HashMap::with_capacity_and_hasher(10, Default::default()))
    }
}


impl<T: Hash + Eq> Foo<i8> for HashSet<T> {
    fn make() -> (Self, Self) {
        (HashSet::new(), HashSet::with_capacity(10))
    }
}
impl Foo<i16> for HashSet<String> {
    fn make() -> (Self, Self) {
        (HashSet::new(), HashSet::with_capacity(10))
    }
}

impl<T: Hash + Eq, S: BuildHasher + Default> Foo<i32> for HashSet<T, S> {
    fn make() -> (Self, Self) {
        (HashSet::default(), HashSet::with_capacity_and_hasher(10, Default::default()))
    }
}
impl<S: BuildHasher + Default> Foo<i64> for HashSet<String, S> {
    fn make() -> (Self, Self) {
        (HashSet::default(), HashSet::with_capacity_and_hasher(10, Default::default()))
    }
}

pub fn foo(_map: &mut HashMap<i32, i32>, _set: &mut HashSet<i32>) {
}

macro_rules! gen {
    (impl) => {
        impl<K: Hash + Eq, V> Foo<u8> for HashMap<K, V> {
            fn make() -> (Self, Self) {
                (HashMap::new(), HashMap::with_capacity(10))
            }
        }
    };

    (fn $name:ident) => {
        pub fn $name(_map: &mut HashMap<i32, i32>, _set: &mut HashSet<i32>) {
        }
    }
}

gen!(impl);
gen!(fn bar);

fn main() {}
