// Copyright 2012 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// ignore-gnu
// ignore-android
// ignore-bitrig
// ignore-macos
// ignore-dragonfly
// ignore-freebsd
// ignore-haiku
// ignore-ios
// ignore-linux
// ignore-netbsd
// ignore-openbsd
// ignore-solaris
// ignore-emscripten

// compile-flags: -Z parse-only

mod not_a_real_file; //~ ERROR file not found for module `not_a_real_file`
//~^ HELP name the file either not_a_real_file.rs or not_a_real_file\mod.rs inside the directory

fn main() {
    assert_eq!(mod_file_aux::bar(), 10);
}
