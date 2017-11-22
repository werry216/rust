// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! See `README.md` for high-level documentation

use hir::def_id::{DefId, LOCAL_CRATE};
use syntax_pos::DUMMY_SP;
use traits::{self, Normalized, SelectionContext, Obligation, ObligationCause, Reveal};
use traits::select::IntercrateAmbiguityCause;
use ty::{self, Ty, TyCtxt};
use ty::subst::Subst;

use infer::{InferCtxt, InferOk};

#[derive(Copy, Clone, Debug)]
enum InferIsLocal {
    BrokenYes,
    Yes,
    No
}

#[derive(Debug, Copy, Clone)]
pub enum Conflict {
    Upstream,
    Downstream
}

pub struct OverlapResult<'tcx> {
    pub impl_header: ty::ImplHeader<'tcx>,
    pub intercrate_ambiguity_causes: Vec<IntercrateAmbiguityCause>,
}

/// If there are types that satisfy both impls, returns a suitably-freshened
/// `ImplHeader` with those types substituted
pub fn overlapping_impls<'cx, 'gcx, 'tcx>(infcx: &InferCtxt<'cx, 'gcx, 'tcx>,
                                          impl1_def_id: DefId,
                                          impl2_def_id: DefId)
                                          -> Option<OverlapResult<'tcx>>
{
    debug!("impl_can_satisfy(\
           impl1_def_id={:?}, \
           impl2_def_id={:?})",
           impl1_def_id,
           impl2_def_id);

    let selcx = &mut SelectionContext::intercrate(infcx);
    overlap(selcx, impl1_def_id, impl2_def_id)
}

fn with_fresh_ty_vars<'cx, 'gcx, 'tcx>(selcx: &mut SelectionContext<'cx, 'gcx, 'tcx>,
                                       param_env: ty::ParamEnv<'tcx>,
                                       impl_def_id: DefId)
                                       -> ty::ImplHeader<'tcx>
{
    let tcx = selcx.tcx();
    let impl_substs = selcx.infcx().fresh_substs_for_item(DUMMY_SP, impl_def_id);

    let header = ty::ImplHeader {
        impl_def_id,
        self_ty: tcx.type_of(impl_def_id),
        trait_ref: tcx.impl_trait_ref(impl_def_id),
        predicates: tcx.predicates_of(impl_def_id).predicates
    }.subst(tcx, impl_substs);

    let Normalized { value: mut header, obligations } =
        traits::normalize(selcx, param_env, ObligationCause::dummy(), &header);

    header.predicates.extend(obligations.into_iter().map(|o| o.predicate));
    header
}

/// Can both impl `a` and impl `b` be satisfied by a common type (including
/// `where` clauses)? If so, returns an `ImplHeader` that unifies the two impls.
fn overlap<'cx, 'gcx, 'tcx>(selcx: &mut SelectionContext<'cx, 'gcx, 'tcx>,
                            a_def_id: DefId,
                            b_def_id: DefId)
                            -> Option<OverlapResult<'tcx>>
{
    debug!("overlap(a_def_id={:?}, b_def_id={:?})",
           a_def_id,
           b_def_id);

    // For the purposes of this check, we don't bring any skolemized
    // types into scope; instead, we replace the generic types with
    // fresh type variables, and hence we do our evaluations in an
    // empty environment.
    let param_env = ty::ParamEnv::empty(Reveal::UserFacing);

    let a_impl_header = with_fresh_ty_vars(selcx, param_env, a_def_id);
    let b_impl_header = with_fresh_ty_vars(selcx, param_env, b_def_id);

    debug!("overlap: a_impl_header={:?}", a_impl_header);
    debug!("overlap: b_impl_header={:?}", b_impl_header);

    // Do `a` and `b` unify? If not, no overlap.
    let obligations = match selcx.infcx().at(&ObligationCause::dummy(), param_env)
                                         .eq_impl_headers(&a_impl_header, &b_impl_header) {
        Ok(InferOk { obligations, value: () }) => {
            obligations
        }
        Err(_) => return None
    };

    debug!("overlap: unification check succeeded");

    // Are any of the obligations unsatisfiable? If so, no overlap.
    let infcx = selcx.infcx();
    let opt_failing_obligation =
        a_impl_header.predicates
                     .iter()
                     .chain(&b_impl_header.predicates)
                     .map(|p| infcx.resolve_type_vars_if_possible(p))
                     .map(|p| Obligation { cause: ObligationCause::dummy(),
                                           param_env,
                                           recursion_depth: 0,
                                           predicate: p })
                     .chain(obligations)
                     .find(|o| !selcx.evaluate_obligation(o));

    if let Some(failing_obligation) = opt_failing_obligation {
        debug!("overlap: obligation unsatisfiable {:?}", failing_obligation);
        return None
    }

    Some(OverlapResult {
        impl_header: selcx.infcx().resolve_type_vars_if_possible(&a_impl_header),
        intercrate_ambiguity_causes: selcx.intercrate_ambiguity_causes().to_vec(),
    })
}

pub fn trait_ref_is_knowable<'a, 'gcx, 'tcx>(tcx: TyCtxt<'a, 'gcx, 'tcx>,
                                             trait_ref: ty::TraitRef<'tcx>,
                                             broken: bool)
                                             -> Option<Conflict>
{
    debug!("trait_ref_is_knowable(trait_ref={:?}, broken={:?})", trait_ref, broken);
    let mode = if broken {
        InferIsLocal::BrokenYes
    } else {
        InferIsLocal::Yes
    };
    if orphan_check_trait_ref(tcx, trait_ref, mode).is_ok() {
        // A downstream or cousin crate is allowed to implement some
        // substitution of this trait-ref.
        debug!("trait_ref_is_knowable: downstream crate might implement");
        return Some(Conflict::Downstream);
    }

    if trait_ref_is_local_or_fundamental(tcx, trait_ref) {
        // This is a local or fundamental trait, so future-compatibility
        // is no concern. We know that downstream/cousin crates are not
        // allowed to implement a substitution of this trait ref, which
        // means impls could only come from dependencies of this crate,
        // which we already know about.
        return None;
    }
    // This is a remote non-fundamental trait, so if another crate
    // can be the "final owner" of a substitution of this trait-ref,
    // they are allowed to implement it future-compatibly.
    //
    // However, if we are a final owner, then nobody else can be,
    // and if we are an intermediate owner, then we don't care
    // about future-compatibility, which means that we're OK if
    // we are an owner.
    if orphan_check_trait_ref(tcx, trait_ref, InferIsLocal::No).is_ok() {
        debug!("trait_ref_is_knowable: orphan check passed");
        return None;
    } else {
        debug!("trait_ref_is_knowable: nonlocal, nonfundamental, unowned");
        return Some(Conflict::Upstream);
    }
}

pub fn trait_ref_is_local_or_fundamental<'a, 'gcx, 'tcx>(tcx: TyCtxt<'a, 'gcx, 'tcx>,
                                                         trait_ref: ty::TraitRef<'tcx>)
                                                         -> bool {
    trait_ref.def_id.krate == LOCAL_CRATE || tcx.has_attr(trait_ref.def_id, "fundamental")
}

pub enum OrphanCheckErr<'tcx> {
    NoLocalInputType,
    UncoveredTy(Ty<'tcx>),
}

/// Checks the coherence orphan rules. `impl_def_id` should be the
/// def-id of a trait impl. To pass, either the trait must be local, or else
/// two conditions must be satisfied:
///
/// 1. All type parameters in `Self` must be "covered" by some local type constructor.
/// 2. Some local type must appear in `Self`.
pub fn orphan_check<'a, 'gcx, 'tcx>(tcx: TyCtxt<'a, 'gcx, 'tcx>,
                                    impl_def_id: DefId)
                                    -> Result<(), OrphanCheckErr<'tcx>>
{
    debug!("orphan_check({:?})", impl_def_id);

    // We only except this routine to be invoked on implementations
    // of a trait, not inherent implementations.
    let trait_ref = tcx.impl_trait_ref(impl_def_id).unwrap();
    debug!("orphan_check: trait_ref={:?}", trait_ref);

    // If the *trait* is local to the crate, ok.
    if trait_ref.def_id.is_local() {
        debug!("trait {:?} is local to current crate",
               trait_ref.def_id);
        return Ok(());
    }

    orphan_check_trait_ref(tcx, trait_ref, InferIsLocal::No)
}

fn orphan_check_trait_ref<'tcx>(tcx: TyCtxt,
                                trait_ref: ty::TraitRef<'tcx>,
                                infer_is_local: InferIsLocal)
                                -> Result<(), OrphanCheckErr<'tcx>>
{
    debug!("orphan_check_trait_ref(trait_ref={:?}, infer_is_local={:?})",
           trait_ref, infer_is_local);

    // First, create an ordered iterator over all the type parameters to the trait, with the self
    // type appearing first.
    // Find the first input type that either references a type parameter OR
    // some local type.
    for input_ty in trait_ref.input_types() {
        if ty_is_local(tcx, input_ty, infer_is_local) {
            debug!("orphan_check_trait_ref: ty_is_local `{:?}`", input_ty);

            // First local input type. Check that there are no
            // uncovered type parameters.
            let uncovered_tys = uncovered_tys(tcx, input_ty, infer_is_local);
            for uncovered_ty in uncovered_tys {
                if let Some(param) = uncovered_ty.walk()
                    .find(|t| is_possibly_remote_type(t, infer_is_local))
                {
                    debug!("orphan_check_trait_ref: uncovered type `{:?}`", param);
                    return Err(OrphanCheckErr::UncoveredTy(param));
                }
            }

            // OK, found local type, all prior types upheld invariant.
            return Ok(());
        }

        // Otherwise, enforce invariant that there are no type
        // parameters reachable.
        if let Some(param) = input_ty.walk()
            .find(|t| is_possibly_remote_type(t, infer_is_local))
        {
            debug!("orphan_check_trait_ref: uncovered type `{:?}`", param);
            return Err(OrphanCheckErr::UncoveredTy(param));
        }
    }

    // If we exit above loop, never found a local type.
    debug!("orphan_check_trait_ref: no local type");
    return Err(OrphanCheckErr::NoLocalInputType);
}

fn uncovered_tys<'tcx>(tcx: TyCtxt, ty: Ty<'tcx>, infer_is_local: InferIsLocal)
                       -> Vec<Ty<'tcx>> {
    if ty_is_local_constructor(ty, infer_is_local) {
        vec![]
    } else if fundamental_ty(tcx, ty) {
        ty.walk_shallow()
          .flat_map(|t| uncovered_tys(tcx, t, infer_is_local))
          .collect()
    } else {
        vec![ty]
    }
}

fn is_possibly_remote_type(ty: Ty, _infer_is_local: InferIsLocal) -> bool {
    match ty.sty {
        ty::TyProjection(..) | ty::TyParam(..) => true,
        _ => false,
    }
}

fn ty_is_local(tcx: TyCtxt, ty: Ty, infer_is_local: InferIsLocal) -> bool {
    ty_is_local_constructor(ty, infer_is_local) ||
        fundamental_ty(tcx, ty) && ty.walk_shallow().any(|t| ty_is_local(tcx, t, infer_is_local))
}

fn fundamental_ty(tcx: TyCtxt, ty: Ty) -> bool {
    match ty.sty {
        ty::TyRef(..) => true,
        ty::TyAdt(def, _) => def.is_fundamental(),
        ty::TyDynamic(ref data, ..) => {
            data.principal().map_or(false, |p| tcx.has_attr(p.def_id(), "fundamental"))
        }
        _ => false
    }
}

fn def_id_is_local(def_id: DefId, infer_is_local: InferIsLocal) -> bool {
    match infer_is_local {
        InferIsLocal::Yes => false,
        InferIsLocal::No |
        InferIsLocal::BrokenYes => def_id.is_local()
    }
}

fn ty_is_local_constructor(ty: Ty, infer_is_local: InferIsLocal) -> bool {
    debug!("ty_is_local_constructor({:?})", ty);

    match ty.sty {
        ty::TyBool |
        ty::TyChar |
        ty::TyInt(..) |
        ty::TyUint(..) |
        ty::TyFloat(..) |
        ty::TyStr |
        ty::TyFnDef(..) |
        ty::TyFnPtr(_) |
        ty::TyArray(..) |
        ty::TySlice(..) |
        ty::TyRawPtr(..) |
        ty::TyRef(..) |
        ty::TyNever |
        ty::TyTuple(..) |
        ty::TyParam(..) |
        ty::TyProjection(..) => {
            false
        }

        ty::TyInfer(..) => match infer_is_local {
            InferIsLocal::No => false,
            InferIsLocal::Yes |
            InferIsLocal::BrokenYes => true
        },

        ty::TyAdt(def, _) => def_id_is_local(def.did, infer_is_local),
        ty::TyForeign(did) => def_id_is_local(did, infer_is_local),

        ty::TyDynamic(ref tt, ..) => {
            tt.principal().map_or(false, |p| {
                def_id_is_local(p.def_id(), infer_is_local)
            })
        }

        ty::TyError => {
            true
        }

        ty::TyClosure(..) | ty::TyGenerator(..) | ty::TyAnon(..) => {
            bug!("ty_is_local invoked on unexpected type: {:?}", ty)
        }
    }
}
