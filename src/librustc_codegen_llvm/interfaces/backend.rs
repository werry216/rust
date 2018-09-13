// Copyright 2018 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use rustc::ty::layout::{HasTyCtxt, LayoutOf, TyLayout};
use rustc::ty::Ty;
use std::fmt::Debug;

pub trait BackendTypes {
    type Value: Debug + PartialEq + Copy;
    type BasicBlock;
    type Type: Debug + PartialEq + Copy;
    type Context;
}

pub trait Backend<'tcx>:
    BackendTypes + HasTyCtxt<'tcx> + LayoutOf<Ty = Ty<'tcx>, TyLayout = TyLayout<'tcx>>
{
}

impl<'tcx, T> Backend<'tcx> for T where
    Self: BackendTypes + HasTyCtxt<'tcx> + LayoutOf<Ty = Ty<'tcx>, TyLayout = TyLayout<'tcx>>
{}
