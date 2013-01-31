// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

struct invariant {
    f: @[mut &int]
}

fn to_same_lifetime(bi: invariant/&r) {
    let bj: invariant/&r = bi;
}

fn to_shorter_lifetime(bi: invariant/&r) {
    let bj: invariant/&blk = bi; //~ ERROR mismatched types
}

fn to_longer_lifetime(bi: invariant/&r) -> invariant/&static {
    bi //~ ERROR mismatched types
}

fn main() {
}