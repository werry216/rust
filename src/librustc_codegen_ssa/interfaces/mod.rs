// Copyright 2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

mod backend;
mod declare;
mod misc;
mod statics;

pub use self::backend::{Backend, BackendMethods, BackendTypes};
pub use self::declare::{DeclareMethods, PreDefineMethods};
pub use self::misc::MiscMethods;
pub use self::statics::StaticMethods;

use std::fmt;

pub trait CodegenObject: Copy + PartialEq + fmt::Debug {}
impl<T: Copy + PartialEq + fmt::Debug> CodegenObject for T {}
