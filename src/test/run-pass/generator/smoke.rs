// Copyright 2017 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// compile-flags: --test

#![feature(generators, generator_trait)]

use std::ops::{State, Generator};
use std::thread;

#[test]
fn simple() {
    let mut foo = || {
        if false {
            yield;
        }
    };

    match foo.resume(()) {
        State::Complete(()) => {}
        s => panic!("bad state: {:?}", s),
    }
}

#[test]
fn return_capture() {
    let a = String::from("foo");
    let mut foo = || {
        if false {
            yield;
        }
        a
    };

    match foo.resume(()) {
        State::Complete(ref s) if *s == "foo" => {}
        s => panic!("bad state: {:?}", s),
    }
}

#[test]
fn simple_yield() {
    let mut foo = || {
        yield;
    };

    match foo.resume(()) {
        State::Yielded(()) => {}
        s => panic!("bad state: {:?}", s),
    }
    match foo.resume(()) {
        State::Complete(()) => {}
        s => panic!("bad state: {:?}", s),
    }
}

#[test]
fn yield_capture() {
    let b = String::from("foo");
    let mut foo = || {
        yield b;
    };

    match foo.resume(()) {
        State::Yielded(ref s) if *s == "foo" => {}
        s => panic!("bad state: {:?}", s),
    }
    match foo.resume(()) {
        State::Complete(()) => {}
        s => panic!("bad state: {:?}", s),
    }
}

#[test]
fn simple_yield_value() {
    let mut foo = || {
        yield String::from("bar");
        return String::from("foo")
    };

    match foo.resume(()) {
        State::Yielded(ref s) if *s == "bar" => {}
        s => panic!("bad state: {:?}", s),
    }
    match foo.resume(()) {
        State::Complete(ref s) if *s == "foo" => {}
        s => panic!("bad state: {:?}", s),
    }
}

#[test]
fn return_after_yield() {
    let a = String::from("foo");
    let mut foo = || {
        yield;
        return a
    };

    match foo.resume(()) {
        State::Yielded(()) => {}
        s => panic!("bad state: {:?}", s),
    }
    match foo.resume(()) {
        State::Complete(ref s) if *s == "foo" => {}
        s => panic!("bad state: {:?}", s),
    }
}

#[test]
fn send_and_sync() {
    assert_send_sync(|| {
        let _: () = gen arg;
        yield
    });
    assert_send_sync(|| {
        let _: () = gen arg;
        yield String::from("foo");
    });
    assert_send_sync(|| {
        let _: () = gen arg;
        yield;
        return String::from("foo");
    });
    let a = 3;
    assert_send_sync(|| {
        let _: () = gen arg;
        yield a;
        return
    });
    let a = 3;
    assert_send_sync(move || {
        let _: () = gen arg;
        yield a;
        return
    });
    let a = String::from("a");
    assert_send_sync(|| {
        let _: () = gen arg;
        yield ;
        drop(a);
        return
    });
    let a = String::from("a");
    assert_send_sync(move || {
        let _: () = gen arg;
        yield ;
        drop(a);
        return
    });

    fn assert_send_sync<T: Send + Sync>(_: T) {}
}

#[test]
fn send_over_threads() {
    let mut foo = || { yield };
    thread::spawn(move || {
        match foo.resume(()) {
            State::Yielded(()) => {}
            s => panic!("bad state: {:?}", s),
        }
        match foo.resume(()) {
            State::Complete(()) => {}
            s => panic!("bad state: {:?}", s),
        }
    }).join().unwrap();

    let a = String::from("a");
    let mut foo = || { yield a };
    thread::spawn(move || {
        match foo.resume(()) {
            State::Yielded(ref s) if *s == "a" => {}
            s => panic!("bad state: {:?}", s),
        }
        match foo.resume(()) {
            State::Complete(()) => {}
            s => panic!("bad state: {:?}", s),
        }
    }).join().unwrap();
}
