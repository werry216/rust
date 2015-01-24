// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// compile-flags:--cfg set1 --cfg set2
#![allow(dead_code)]
use std::fmt::Debug;

struct NotDebugable;

#[cfg_attr(set1, derive(Debug))]
struct Set1;

#[cfg_attr(notset, derive(Debug))]
struct Notset(NotDebugable);

#[cfg_attr(not(notset), derive(Debug))]
struct NotNotset;

#[cfg_attr(not(set1), derive(Debug))]
struct NotSet1(NotDebugable);

#[cfg_attr(all(set1, set2), derive(Debug))]
struct AllSet1Set2;

#[cfg_attr(all(set1, notset), derive(Debug))]
struct AllSet1Notset(NotDebugable);

#[cfg_attr(any(set1, notset), derive(Debug))]
struct AnySet1Notset;

#[cfg_attr(any(notset, notset2), derive(Debug))]
struct AnyNotsetNotset2(NotDebugable);

#[cfg_attr(all(not(notset), any(set1, notset)), derive(Debug))]
struct Complex;

#[cfg_attr(any(notset, not(any(set1, notset))), derive(Debug))]
struct ComplexNot(NotDebugable);

#[cfg_attr(any(target_endian = "little", target_endian = "big"), derive(Debug))]
struct KeyValue;

fn is_show<T: Debug>() {}

fn main() {
    is_show::<Set1>();
    is_show::<NotNotset>();
    is_show::<AllSet1Set2>();
    is_show::<AnySet1Notset>();
    is_show::<Complex>();
    is_show::<KeyValue>();
}
