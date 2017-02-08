// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use dep_graph::DepNode;
use hir::def_id::DefId;
use ty::{self, Ty, TypeFoldable, Substs};
use util::ppaux;

use std::borrow::Cow;
use std::fmt;
use syntax::ast;


#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Instance<'tcx> {
    pub def: InstanceDef<'tcx>,
    pub substs: &'tcx Substs<'tcx>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum InstanceDef<'tcx> {
    Item(DefId),
    // <fn() as FnTrait>::call_*
    FnPtrShim(DefId, Ty<'tcx>),
}

impl<'tcx> InstanceDef<'tcx> {
    #[inline]
    pub fn def_id(&self) -> DefId {
        match *self {
            InstanceDef::Item(def_id) |
            InstanceDef::FnPtrShim(def_id, _)
                => def_id
        }
    }

    #[inline]
    pub fn def_ty<'a>(&self, tcx: ty::TyCtxt<'a, 'tcx, 'tcx>) -> Ty<'tcx> {
        tcx.item_type(self.def_id())
    }

    #[inline]
    pub fn attrs<'a>(&self, tcx: ty::TyCtxt<'a, 'tcx, 'tcx>) -> Cow<'tcx, [ast::Attribute]> {
        tcx.get_attrs(self.def_id())
    }

    pub(crate) fn dep_node(&self) -> DepNode<DefId> {
        // HACK: def-id binning, project-style; someone replace this with
        // real on-demand.
        let ty = match self {
            &InstanceDef::FnPtrShim(_, ty) => Some(ty),
            _ => None
        }.into_iter();

        DepNode::MirShim(
            Some(self.def_id()).into_iter().chain(
                ty.flat_map(|t| t.walk()).flat_map(|t| match t.sty {
                   ty::TyAdt(adt_def, _) => Some(adt_def.did),
                   ty::TyProjection(ref proj) => Some(proj.trait_ref.def_id),
                   _ => None,
               })
            ).collect()
        )
    }
}

impl<'tcx> fmt::Display for Instance<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.def {
            InstanceDef::Item(def) => {
                ppaux::parameterized(f, self.substs, def, &[])
            }
            InstanceDef::FnPtrShim(def, ty) => {
                ppaux::parameterized(f, self.substs, def, &[])?;
                write!(f, " - shim({:?})", ty)
            }
        }
    }
}

impl<'a, 'b, 'tcx> Instance<'tcx> {
    pub fn new(def_id: DefId, substs: &'tcx Substs<'tcx>)
               -> Instance<'tcx> {
        assert!(substs.is_normalized_for_trans() && !substs.has_escaping_regions(),
                "substs of instance {:?} not normalized for trans: {:?}",
                def_id, substs);
        Instance { def: InstanceDef::Item(def_id), substs: substs }
    }

    pub fn mono(tcx: ty::TyCtxt<'a, 'tcx, 'b>, def_id: DefId) -> Instance<'tcx> {
        Instance::new(def_id, tcx.global_tcx().empty_substs_for_def_id(def_id))
    }

    #[inline]
    pub fn def_id(&self) -> DefId {
        self.def.def_id()
    }
}
