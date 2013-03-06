// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// xfail-fast

use core::container::{Container, Mutable, Map};
use core::iter::BaseIter;

enum cat_type { tuxedo, tabby, tortoiseshell }

impl cmp::Eq for cat_type {
    pure fn eq(&self, other: &cat_type) -> bool {
        ((*self) as uint) == ((*other) as uint)
    }
    pure fn ne(&self, other: &cat_type) -> bool { !(*self).eq(other) }
}

// Very silly -- this just returns the value of the name field
// for any int value that's less than the meows field

// ok: T should be in scope when resolving the trait ref for map
struct cat<T> {
    // Yes, you can have negative meows
    priv meows : int,

    how_hungry : int,
    name : T,
}

pub impl<T> cat<T> {
    fn speak(&mut self) { self.meow(); }

    fn eat(&mut self) -> bool {
        if self.how_hungry > 0 {
            error!("OM NOM NOM");
            self.how_hungry -= 2;
            return true;
        } else {
            error!("Not hungry!");
            return false;
        }
    }
}

impl<T> BaseIter<(int, &'self T)> for cat<T> {
    pure fn each(&self, f: fn(&(int, &'self T)) -> bool) {
        let mut n = int::abs(self.meows);
        while n > 0 {
            if !f(&(n, &self.name)) { break; }
            n -= 1;
        }
    }

    pure fn size_hint(&self) -> Option<uint> { Some(self.len()) }
}

impl<T> Container for cat<T> {
    pure fn len(&self) -> uint { self.meows as uint }
    pure fn is_empty(&self) -> bool { self.meows == 0 }
}

impl<T> Mutable for cat<T> {
    fn clear(&mut self) {}
}

impl<T> Map<int, T> for cat<T> {
    pure fn contains_key(&self, k: &int) -> bool { *k <= self.meows }

    pure fn each_key(&self, f: fn(v: &int) -> bool) {
        for self.each |&(k, _)| { if !f(&k) { break; } loop;};
    }

    pure fn each_value(&self, f: fn(v: &T) -> bool) {
        for self.each |&(_, v)| { if !f(v) { break; } loop;};
    }

    fn insert(&mut self, k: int, _: T) -> bool {
        self.meows += k;
        true
    }

    pure fn find(&self, k: &int) -> Option<&'self T> {
        if *k <= self.meows {
            Some(&self.name)
        } else {
            None
        }
    }

    fn remove(&mut self, k: &int) -> bool {
        if self.find(k).is_some() {
            self.meows -= *k; true
        } else {
            false
        }
    }
}

pub impl<T> cat<T> {
    pure fn get(&self, k: &int) -> &'self T {
        match self.find(k) {
          Some(v) => { v }
          None    => { fail!(~"epic fail"); }
        }
    }

    static pure fn new(in_x: int, in_y: int, in_name: T) -> cat<T> {
        cat{meows: in_x, how_hungry: in_y, name: in_name }
    }
}

priv impl<T> cat<T> {
    fn meow(&mut self) {
        self.meows += 1;
        error!("Meow %d", self.meows);
        if self.meows % 5 == 0 {
            self.how_hungry += 1;
        }
    }
}

pub fn main() {
    let mut nyan: cat<~str> = cat::new(0, 2, ~"nyan");
    for uint::range(1, 5) |_| { nyan.speak(); }
    fail_unless!((*nyan.find(&1).unwrap() == ~"nyan"));
    fail_unless!((nyan.find(&10) == None));
    let mut spotty: cat<cat_type> = cat::new(2, 57, tuxedo);
    for uint::range(0, 6) |_| { spotty.speak(); }
    fail_unless!((spotty.len() == 8));
    fail_unless!((spotty.contains_key(&2)));
    fail_unless!((spotty.get(&3) == &tuxedo));
}
