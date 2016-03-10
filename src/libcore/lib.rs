// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! # The Rust Core Library
//!
//! The Rust Core Library is the dependency-free[^free] foundation of [The
//! Rust Standard Library](../std/index.html). It is the portable glue
//! between the language and its libraries, defining the intrinsic and
//! primitive building blocks of all Rust code. It links to no
//! upstream libraries, no system libraries, and no libc.
//!
//! [^free]: Strictly speaking, there are some symbols which are needed but
//!          they aren't always necessary.
//!
//! The core library is *minimal*: it isn't even aware of heap allocation,
//! nor does it provide concurrency or I/O. These things require
//! platform integration, and this library is platform-agnostic.
//!
//! # How to use the core library
//!
// FIXME: Fill me in with more detail when the interface settles
//! This library is built on the assumption of a few existing symbols:
//!
//! * `memcpy`, `memcmp`, `memset` - These are core memory routines which are
//!   often generated by LLVM. Additionally, this library can make explicit
//!   calls to these functions. Their signatures are the same as found in C.
//!   These functions are often provided by the system libc, but can also be
//!   provided by the [rlibc crate](https://crates.io/crates/rlibc).
//!
//! * `rust_begin_unwind` - This function takes three arguments, a
//!   `fmt::Arguments`, a `&str`, and a `u32`. These three arguments dictate
//!   the panic message, the file at which panic was invoked, and the line.
//!   It is up to consumers of this core library to define this panic
//!   function; it is only required to never return.

// Since libcore defines many fundamental lang items, all tests live in a
// separate crate, libcoretest, to avoid bizarre issues.

#![crate_name = "core"]
#![stable(feature = "core", since = "1.6.0")]
#![crate_type = "rlib"]
#![doc(html_logo_url = "https://www.rust-lang.org/logos/rust-logo-128x128-blk-v2.png",
       html_favicon_url = "https://doc.rust-lang.org/favicon.ico",
       html_root_url = "https://doc.rust-lang.org/nightly/",
       html_playground_url = "https://play.rust-lang.org/",
       issue_tracker_base_url = "https://github.com/rust-lang/rust/issues/",
       test(no_crate_inject, attr(deny(warnings))),
       test(attr(allow(dead_code, deprecated, unused_variables, unused_mut))))]

#![no_core]
#![deny(missing_docs)]
#![cfg_attr(not(stage0), deny(warnings))]

#![feature(allow_internal_unstable)]
#![feature(associated_type_defaults)]
#![feature(concat_idents)]
#![feature(const_fn)]
#![feature(custom_attribute)]
#![feature(fundamental)]
#![feature(intrinsics)]
#![feature(lang_items)]
#![feature(no_core)]
#![feature(on_unimplemented)]
#![feature(optin_builtin_traits)]
#![feature(reflect)]
#![feature(unwind_attributes)]
#![feature(repr_simd, platform_intrinsics)]
#![feature(rustc_attrs)]
#![feature(staged_api)]
#![feature(unboxed_closures)]

#[macro_use]
mod macros;

#[path = "num/float_macros.rs"]
#[macro_use]
mod float_macros;

#[path = "num/int_macros.rs"]
#[macro_use]
mod int_macros;

#[path = "num/uint_macros.rs"]
#[macro_use]
mod uint_macros;

#[path = "num/isize.rs"]  pub mod isize;
#[path = "num/i8.rs"]   pub mod i8;
#[path = "num/i16.rs"]  pub mod i16;
#[path = "num/i32.rs"]  pub mod i32;
#[path = "num/i64.rs"]  pub mod i64;

#[path = "num/usize.rs"] pub mod usize;
#[path = "num/u8.rs"]   pub mod u8;
#[path = "num/u16.rs"]  pub mod u16;
#[path = "num/u32.rs"]  pub mod u32;
#[path = "num/u64.rs"]  pub mod u64;

#[path = "num/f32.rs"]   pub mod f32;
#[path = "num/f64.rs"]   pub mod f64;

#[macro_use]
pub mod num;

/* The libcore prelude, not as all-encompassing as the libstd prelude */

pub mod prelude;

/* Core modules for ownership management */

pub mod intrinsics;
pub mod mem;
pub mod nonzero;
pub mod ptr;

/* Core language traits */

pub mod marker;
pub mod ops;
pub mod cmp;
pub mod clone;
pub mod default;
pub mod convert;
pub mod borrow;

/* Core types and methods on primitives */

pub mod any;
pub mod array;
pub mod sync;
pub mod cell;
pub mod char;
pub mod panicking;
pub mod iter;
pub mod option;
pub mod raw;
pub mod result;

pub mod slice;
pub mod str;
pub mod hash;
pub mod fmt;

// note: does not need to be public
mod tuple;
