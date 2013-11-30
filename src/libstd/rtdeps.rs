// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! This module contains the linkage attributes to all runtime dependencies of
//! the stndard library This varies per-platform, but these libraries are
//! necessary for running libstd.

// All platforms need to link to rustrt
#[link(name = "rustrt", kind = "static")]
extern {}

// LLVM implements the `frem` instruction as a call to `fmod`, which lives in
// libm. Hence, we must explicitly link to it.
//
// On linux librt and libdl are indirect dependencies via rustrt,
// and binutils 2.22+ won't add them automatically
#[cfg(target_os = "linux")]
#[link(name = "rt")]
#[link(name = "dl")]
#[link(name = "m")]
#[link(name = "pthread")]
#[link(name = "stdc++")]
extern {}

#[cfg(target_os = "android")]
#[link(name = "dl")]
#[link(name = "log")]
#[link(name = "supc++")]
#[link(name = "gnustl_shared")]
#[link(name = "m")]
extern {}

#[cfg(target_os = "freebsd")]
#[link(name = "execinfo")]
#[link(name = "rt")]
#[link(name = "stdc++")]
#[link(name = "pthread")]
extern {}

#[cfg(target_os = "macos")]
#[link(name = "pthread")]
#[link(name = "stdc++")]
extern {}

#[cfg(stage0)]
#[link_args = "-lstdc++"]
extern {}
