// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use hir::def_id::DefId;
use ty::{self, Ty, TypeFoldable, Substs, TyCtxt};
use ty::subst::{Kind, Subst};
use traits;
use syntax::abi::Abi;
use util::ppaux;

use std::fmt;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Instance<'tcx> {
    pub def: InstanceDef<'tcx>,
    pub substs: &'tcx Substs<'tcx>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum InstanceDef<'tcx> {
    Item(DefId),
    Intrinsic(DefId),

    /// \<fn() as FnTrait>::call_*
    /// def-id is FnTrait::call_*
    FnPtrShim(DefId, Ty<'tcx>),

    /// <Trait as Trait>::fn
    Virtual(DefId, usize),

    /// <[mut closure] as FnOnce>::call_once
    ClosureOnceShim { call_once: DefId },

    /// drop_in_place::<T>; None for empty drop glue.
    DropGlue(DefId, Option<Ty<'tcx>>),

    ///`<T as Clone>::clone` shim.
    CloneShim(DefId, Ty<'tcx>),
}

impl<'a, 'tcx> Instance<'tcx> {
    pub fn ty(&self,
              tcx: TyCtxt<'a, 'tcx, 'tcx>)
              -> Ty<'tcx>
    {
        let ty = tcx.type_of(self.def.def_id());
        tcx.trans_apply_param_substs(self.substs, &ty)
    }
}

impl<'tcx> InstanceDef<'tcx> {
    #[inline]
    pub fn def_id(&self) -> DefId {
        match *self {
            InstanceDef::Item(def_id) |
            InstanceDef::FnPtrShim(def_id, _) |
            InstanceDef::Virtual(def_id, _) |
            InstanceDef::Intrinsic(def_id, ) |
            InstanceDef::ClosureOnceShim { call_once: def_id } |
            InstanceDef::DropGlue(def_id, _) |
            InstanceDef::CloneShim(def_id, _) => def_id
        }
    }

    #[inline]
    pub fn attrs<'a>(&self, tcx: TyCtxt<'a, 'tcx, 'tcx>) -> ty::Attributes<'tcx> {
        tcx.get_attrs(self.def_id())
    }

    pub fn is_inline<'a>(
        &self,
        tcx: TyCtxt<'a, 'tcx, 'tcx>
    ) -> bool {
        use hir::map::DefPathData;
        let def_id = match *self {
            ty::InstanceDef::Item(def_id) => def_id,
            ty::InstanceDef::DropGlue(_, Some(_)) => return false,
            _ => return true
        };
        match tcx.def_key(def_id).disambiguated_data.data {
            DefPathData::StructCtor |
            DefPathData::EnumVariant(..) |
            DefPathData::ClosureExpr => true,
            _ => false
        }
    }

    pub fn requires_local<'a>(
        &self,
        tcx: TyCtxt<'a, 'tcx, 'tcx>
    ) -> bool {
        use syntax::attr::requests_inline;
        if self.is_inline(tcx) {
            return true
        }
        if let ty::InstanceDef::DropGlue(..) = *self {
            // Drop glue wants to be instantiated at every translation
            // unit, but without an #[inline] hint. We should make this
            // available to normal end-users.
            return true
        }
        requests_inline(&self.attrs(tcx)[..]) ||
            tcx.is_const_fn(self.def_id())
    }
}

impl<'tcx> fmt::Display for Instance<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        ppaux::parameterized(f, self.substs, self.def_id(), &[])?;
        match self.def {
            InstanceDef::Item(_) => Ok(()),
            InstanceDef::Intrinsic(_) => {
                write!(f, " - intrinsic")
            }
            InstanceDef::Virtual(_, num) => {
                write!(f, " - shim(#{})", num)
            }
            InstanceDef::FnPtrShim(_, ty) => {
                write!(f, " - shim({:?})", ty)
            }
            InstanceDef::ClosureOnceShim { .. } => {
                write!(f, " - shim")
            }
            InstanceDef::DropGlue(_, ty) => {
                write!(f, " - shim({:?})", ty)
            }
            InstanceDef::CloneShim(_, ty) => {
                write!(f, " - shim({:?})", ty)
            }
        }
    }
}

impl<'a, 'b, 'tcx> Instance<'tcx> {
    pub fn new(def_id: DefId, substs: &'tcx Substs<'tcx>)
               -> Instance<'tcx> {
        assert!(!substs.has_escaping_regions(),
                "substs of instance {:?} not normalized for trans: {:?}",
                def_id, substs);
        Instance { def: InstanceDef::Item(def_id), substs: substs }
    }

    pub fn mono(tcx: TyCtxt<'a, 'tcx, 'b>, def_id: DefId) -> Instance<'tcx> {
        Instance::new(def_id, tcx.global_tcx().empty_substs_for_def_id(def_id))
    }

    #[inline]
    pub fn def_id(&self) -> DefId {
        self.def.def_id()
    }

    /// Resolve a (def_id, substs) pair to an (optional) instance -- most commonly,
    /// this is used to find the precise code that will run for a trait method invocation,
    /// if known.
    ///
    /// Returns `None` if we cannot resolve `Instance` to a specific instance.
    /// For example, in a context like this,
    ///
    /// ```
    /// fn foo<T: Debug>(t: T) { ... }
    /// ```
    ///
    /// trying to resolve `Debug::fmt` applied to `T` will yield `None`, because we do not
    /// know what code ought to run. (Note that this setting is also affected by the
    /// `RevealMode` in the parameter environment.)
    ///
    /// Presuming that coherence and type-check have succeeded, if this method is invoked
    /// in a monomorphic context (i.e., like during trans), then it is guaranteed to return
    /// `Some`.
    pub fn resolve(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                   param_env: ty::ParamEnv<'tcx>,
                   def_id: DefId,
                   substs: &'tcx Substs<'tcx>) -> Option<Instance<'tcx>> {
        debug!("resolve(def_id={:?}, substs={:?})", def_id, substs);
        let result = if let Some(trait_def_id) = tcx.trait_of_item(def_id) {
            debug!(" => associated item, attempting to find impl in param_env {:#?}", param_env);
            let item = tcx.associated_item(def_id);
            resolve_associated_item(tcx, &item, param_env, trait_def_id, substs)
        } else {
            let ty = tcx.type_of(def_id);
            let item_type = tcx.trans_apply_param_substs_env(substs, param_env, &ty);

            let def = match item_type.sty {
                ty::TyFnDef(..) if {
                    let f = item_type.fn_sig(tcx);
                    f.abi() == Abi::RustIntrinsic ||
                        f.abi() == Abi::PlatformIntrinsic
                } =>
                {
                    debug!(" => intrinsic");
                    ty::InstanceDef::Intrinsic(def_id)
                }
                _ => {
                    if Some(def_id) == tcx.lang_items().drop_in_place_fn() {
                        let ty = substs.type_at(0);
                        if ty.needs_drop(tcx, ty::ParamEnv::empty(traits::Reveal::All)) {
                            debug!(" => nontrivial drop glue");
                            ty::InstanceDef::DropGlue(def_id, Some(ty))
                        } else {
                            debug!(" => trivial drop glue");
                            ty::InstanceDef::DropGlue(def_id, None)
                        }
                    } else {
                        debug!(" => free item");
                        ty::InstanceDef::Item(def_id)
                    }
                }
            };
            Some(Instance {
                def: def,
                substs: substs
            })
        };
        debug!("resolve(def_id={:?}, substs={:?}) = {:?}", def_id, substs, result);
        result
    }

    pub fn resolve_closure(
                    tcx: TyCtxt<'a, 'tcx, 'tcx>,
                    def_id: DefId,
                    substs: ty::ClosureSubsts<'tcx>,
                    requested_kind: ty::ClosureKind)
    -> Instance<'tcx>
    {
        let actual_kind = substs.closure_kind(def_id, tcx);

        match needs_fn_once_adapter_shim(actual_kind, requested_kind) {
            Ok(true) => fn_once_adapter_instance(tcx, def_id, substs),
            _ => Instance::new(def_id, substs.substs)
        }
    }
}

fn resolve_associated_item<'a, 'tcx>(
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    trait_item: &ty::AssociatedItem,
    param_env: ty::ParamEnv<'tcx>,
    trait_id: DefId,
    rcvr_substs: &'tcx Substs<'tcx>,
) -> Option<Instance<'tcx>> {
    let def_id = trait_item.def_id;
    debug!("resolve_associated_item(trait_item={:?}, \
                                    trait_id={:?}, \
           rcvr_substs={:?})",
           def_id, trait_id, rcvr_substs);

    let trait_ref = ty::TraitRef::from_method(tcx, trait_id, rcvr_substs);
    let vtbl = tcx.trans_fulfill_obligation((param_env, ty::Binder(trait_ref)));

    // Now that we know which impl is being used, we can dispatch to
    // the actual function:
    match vtbl {
        traits::VtableImpl(impl_data) => {
            let (def_id, substs) = traits::find_associated_item(
                tcx, trait_item, rcvr_substs, &impl_data);
            let substs = tcx.erase_regions(&substs);
            Some(ty::Instance::new(def_id, substs))
        }
        traits::VtableGenerator(closure_data) => {
            Some(Instance {
                def: ty::InstanceDef::Item(closure_data.closure_def_id),
                substs: closure_data.substs.substs
            })
        }
        traits::VtableClosure(closure_data) => {
            let trait_closure_kind = tcx.lang_items().fn_trait_kind(trait_id).unwrap();
            Some(Instance::resolve_closure(tcx, closure_data.closure_def_id, closure_data.substs,
                                 trait_closure_kind))
        }
        traits::VtableFnPointer(ref data) => {
            Some(Instance {
                def: ty::InstanceDef::FnPtrShim(trait_item.def_id, data.fn_ty),
                substs: rcvr_substs
            })
        }
        traits::VtableObject(ref data) => {
            let index = tcx.get_vtable_index_of_object_method(data, def_id);
            Some(Instance {
                def: ty::InstanceDef::Virtual(def_id, index),
                substs: rcvr_substs
            })
        }
        traits::VtableBuiltin(..) => {
            if let Some(_) = tcx.lang_items().clone_trait() {
                Some(Instance {
                    def: ty::InstanceDef::CloneShim(def_id, trait_ref.self_ty()),
                    substs: rcvr_substs
                })
            } else {
                None
            }
        }
        traits::VtableAutoImpl(..) | traits::VtableParam(..) => None
    }
}

fn needs_fn_once_adapter_shim<'a, 'tcx>(actual_closure_kind: ty::ClosureKind,
                              trait_closure_kind: ty::ClosureKind)
    -> Result<bool, ()>
{
    match (actual_closure_kind, trait_closure_kind) {
        (ty::ClosureKind::Fn, ty::ClosureKind::Fn) |
            (ty::ClosureKind::FnMut, ty::ClosureKind::FnMut) |
            (ty::ClosureKind::FnOnce, ty::ClosureKind::FnOnce) => {
                // No adapter needed.
                Ok(false)
            }
        (ty::ClosureKind::Fn, ty::ClosureKind::FnMut) => {
            // The closure fn `llfn` is a `fn(&self, ...)`.  We want a
            // `fn(&mut self, ...)`. In fact, at trans time, these are
            // basically the same thing, so we can just return llfn.
            Ok(false)
        }
        (ty::ClosureKind::Fn, ty::ClosureKind::FnOnce) |
            (ty::ClosureKind::FnMut, ty::ClosureKind::FnOnce) => {
                // The closure fn `llfn` is a `fn(&self, ...)` or `fn(&mut
                // self, ...)`.  We want a `fn(self, ...)`. We can produce
                // this by doing something like:
                //
                //     fn call_once(self, ...) { call_mut(&self, ...) }
                //     fn call_once(mut self, ...) { call_mut(&mut self, ...) }
                //
                // These are both the same at trans time.
                Ok(true)
        }
        (ty::ClosureKind::FnMut, _) |
        (ty::ClosureKind::FnOnce, _) => Err(())
    }
}

fn fn_once_adapter_instance<'a, 'tcx>(
                            tcx: TyCtxt<'a, 'tcx, 'tcx>,
                            closure_did: DefId,
                            substs: ty::ClosureSubsts<'tcx>,
                            ) -> Instance<'tcx> {
    debug!("fn_once_adapter_shim({:?}, {:?})",
    closure_did,
    substs);
    let fn_once = tcx.lang_items().fn_once_trait().unwrap();
    let call_once = tcx.associated_items(fn_once)
        .find(|it| it.kind == ty::AssociatedKind::Method)
        .unwrap().def_id;
    let def = ty::InstanceDef::ClosureOnceShim { call_once };

    let self_ty = tcx.mk_closure_from_closure_substs(
        closure_did, substs);

    let sig = substs.closure_sig(closure_did, tcx);
    let sig = tcx.erase_late_bound_regions_and_normalize(&sig);
    assert_eq!(sig.inputs().len(), 1);
    let substs = tcx.mk_substs([
                               Kind::from(self_ty),
                               Kind::from(sig.inputs()[0]),
    ].iter().cloned());

    debug!("fn_once_adapter_shim: self_ty={:?} sig={:?}", self_ty, sig);
    Instance { def, substs }
}
