// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Empty struct defined with braces add names into type namespace
// Empty struct defined without braces add names into both type and value namespaces

// aux-build:empty-struct.rs

#![feature(braced_empty_structs)]

extern crate empty_struct;
use empty_struct::*;

struct Empty1 {}
struct Empty2;
struct Empty3 {}
const Empty3: Empty3 = Empty3 {};

enum E {
    Empty4 {},
    Empty5,
}

fn local() {
    let e1: Empty1 = Empty1 {};
    let e2: Empty2 = Empty2 {};
    let e2: Empty2 = Empty2;
    let e3: Empty3 = Empty3 {};
    let e3: Empty3 = Empty3;
    let e4: E = E::Empty4 {};
    let e5: E = E::Empty5 {};
    let e5: E = E::Empty5;

    match e1 {
        Empty1 {} => {}
    }
    match e2 {
        Empty2 {} => {}
    }
    match e3 {
        Empty3 {} => {}
    }
    match e4 {
        E::Empty4 {} => {}
        _ => {}
    }
    match e5 {
        E::Empty5 {} => {}
        _ => {}
    }

    match e1 {
        Empty1 { .. } => {}
    }
    match e2 {
        Empty2 { .. } => {}
    }
    match e3 {
        Empty3 { .. } => {}
    }
    match e4 {
        E::Empty4 { .. } => {}
        _ => {}
    }
    match e5 {
        E::Empty5 { .. } => {}
        _ => {}
    }

    match e2 {
        Empty2 => {}
    }
    match e3 {
        Empty3 => {}
    }
    match e5 {
        E::Empty5 => {}
        _ => {}
    }

    let e11: Empty1 = Empty1 { ..e1 };
    let e22: Empty2 = Empty2 { ..e2 };
    let e33: Empty3 = Empty3 { ..e3 };
}

fn xcrate() {
    let e1: XEmpty1 = XEmpty1 {};
    let e2: XEmpty2 = XEmpty2 {};
    let e2: XEmpty2 = XEmpty2;
    let e3: XE = XE::XEmpty3 {};
    // FIXME: Commented out tests are waiting for PR 30882 (fixes for variant namespaces)
    // let e4: XE = XE::XEmpty4 {};
    let e4: XE = XE::XEmpty4;

    match e1 {
        XEmpty1 {} => {}
    }
    match e2 {
        XEmpty2 {} => {}
    }
    match e3 {
        XE::XEmpty3 {} => {}
        _ => {}
    }
    // match e4 {
    //     XE::XEmpty4 {} => {}
    //     _ => {}
    // }

    match e1 {
        XEmpty1 { .. } => {}
    }
    match e2 {
        XEmpty2 { .. } => {}
    }
    match e3 {
        XE::XEmpty3 { .. } => {}
        _ => {}
    }
    // match e4 {
    //     XE::XEmpty4 { .. } => {}
    //     _ => {}
    // }

    match e2 {
        XEmpty2 => {}
    }
    // match e4 {
    //     XE::XEmpty4 => {}
    //     _ => {}
    // }

    let e11: XEmpty1 = XEmpty1 { ..e1 };
    let e22: XEmpty2 = XEmpty2 { ..e2 };
}

fn main() {
    local();
    xcrate();
}
