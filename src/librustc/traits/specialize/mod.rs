// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Logic and data structures related to impl specialization, explained in
// greater detail below.
//
// At the moment, this implementation support only the simple "chain" rule:
// If any two impls overlap, one must be a strict subset of the other.
//
// See traits/README.md for a bit more detail on how specialization
// fits together with the rest of the trait machinery.

use super::{SelectionContext, FulfillmentContext};
use super::util::impl_trait_ref_and_oblig;

use rustc_data_structures::fx::FxHashMap;
use hir::def_id::DefId;
use infer::{InferCtxt, InferOk};
use ty::subst::{Subst, Substs};
use traits::{self, Reveal, ObligationCause};
use ty::{self, TyCtxt, TypeFoldable};
use syntax_pos::DUMMY_SP;
use std::rc::Rc;

pub mod specialization_graph;

/// Information pertinent to an overlapping impl error.
pub struct OverlapError {
    pub with_impl: DefId,
    pub trait_desc: String,
    pub self_desc: Option<String>,
}

/// Given a subst for the requested impl, translate it to a subst
/// appropriate for the actual item definition (whether it be in that impl,
/// a parent impl, or the trait).
///
/// When we have selected one impl, but are actually using item definitions from
/// a parent impl providing a default, we need a way to translate between the
/// type parameters of the two impls. Here the `source_impl` is the one we've
/// selected, and `source_substs` is a substitution of its generics.
/// And `target_node` is the impl/trait we're actually going to get the
/// definition from. The resulting substitution will map from `target_node`'s
/// generics to `source_impl`'s generics as instantiated by `source_subst`.
///
/// For example, consider the following scenario:
///
/// ```rust
/// trait Foo { ... }
/// impl<T, U> Foo for (T, U) { ... }  // target impl
/// impl<V> Foo for (V, V) { ... }     // source impl
/// ```
///
/// Suppose we have selected "source impl" with `V` instantiated with `u32`.
/// This function will produce a substitution with `T` and `U` both mapping to `u32`.
///
/// Where clauses add some trickiness here, because they can be used to "define"
/// an argument indirectly:
///
/// ```rust
/// impl<'a, I, T: 'a> Iterator for Cloned<I>
///    where I: Iterator<Item=&'a T>, T: Clone
/// ```
///
/// In a case like this, the substitution for `T` is determined indirectly,
/// through associated type projection. We deal with such cases by using
/// *fulfillment* to relate the two impls, requiring that all projections are
/// resolved.
pub fn translate_substs<'a, 'gcx, 'tcx>(infcx: &InferCtxt<'a, 'gcx, 'tcx>,
                                        param_env: ty::ParamEnv<'tcx>,
                                        source_impl: DefId,
                                        source_substs: &'tcx Substs<'tcx>,
                                        target_node: specialization_graph::Node)
                                        -> &'tcx Substs<'tcx> {
    let source_trait_ref = infcx.tcx
                                .impl_trait_ref(source_impl)
                                .unwrap()
                                .subst(infcx.tcx, &source_substs);

    // translate the Self and TyParam parts of the substitution, since those
    // vary across impls
    let target_substs = match target_node {
        specialization_graph::Node::Impl(target_impl) => {
            // no need to translate if we're targeting the impl we started with
            if source_impl == target_impl {
                return source_substs;
            }

            fulfill_implication(infcx, param_env, source_trait_ref, target_impl)
                .unwrap_or_else(|_| {
                    bug!("When translating substitutions for specialization, the expected \
                          specialization failed to hold")
                })
        }
        specialization_graph::Node::Trait(..) => source_trait_ref.substs,
    };

    // directly inherent the method generics, since those do not vary across impls
    source_substs.rebase_onto(infcx.tcx, source_impl, target_substs)
}

/// Given a selected impl described by `impl_data`, returns the
/// definition and substitutions for the method with the name `name`
/// the kind `kind`, and trait method substitutions `substs`, in
/// that impl, a less specialized impl, or the trait default,
/// whichever applies.
pub fn find_associated_item<'a, 'tcx>(
    tcx: TyCtxt<'a, 'tcx, 'tcx>,
    item: &ty::AssociatedItem,
    substs: &'tcx Substs<'tcx>,
    impl_data: &super::VtableImplData<'tcx, ()>,
) -> (DefId, &'tcx Substs<'tcx>) {
    assert!(!substs.needs_infer());

    let trait_def_id = tcx.trait_id_of_impl(impl_data.impl_def_id).unwrap();
    let trait_def = tcx.trait_def(trait_def_id);

    let ancestors = trait_def.ancestors(tcx, impl_data.impl_def_id);
    match ancestors.defs(tcx, item.name, item.kind).next() {
        Some(node_item) => {
            let substs = tcx.infer_ctxt().enter(|infcx| {
                let param_env = ty::ParamEnv::empty(Reveal::All);
                let substs = substs.rebase_onto(tcx, trait_def_id, impl_data.substs);
                let substs = translate_substs(&infcx, param_env, impl_data.impl_def_id,
                                              substs, node_item.node);
                let substs = infcx.tcx.erase_regions(&substs);
                tcx.lift(&substs).unwrap_or_else(|| {
                    bug!("find_method: translate_substs \
                          returned {:?} which contains inference types/regions",
                         substs);
                })
            });
            (node_item.item.def_id, substs)
        }
        None => {
            bug!("{:?} not found in {:?}", item, impl_data.impl_def_id)
        }
    }
}

/// Is impl1 a specialization of impl2?
///
/// Specialization is determined by the sets of types to which the impls apply;
/// impl1 specializes impl2 if it applies to a subset of the types impl2 applies
/// to.
pub fn specializes<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                             impl1_def_id: DefId,
                             impl2_def_id: DefId) -> bool {
    debug!("specializes({:?}, {:?})", impl1_def_id, impl2_def_id);

    if let Some(r) = tcx.specializes_cache.borrow().check(impl1_def_id, impl2_def_id) {
        return r;
    }

    // The feature gate should prevent introducing new specializations, but not
    // taking advantage of upstream ones.
    if !tcx.sess.features.borrow().specialization &&
        (impl1_def_id.is_local() || impl2_def_id.is_local()) {
        return false;
    }

    // We determine whether there's a subset relationship by:
    //
    // - skolemizing impl1,
    // - assuming the where clauses for impl1,
    // - instantiating impl2 with fresh inference variables,
    // - unifying,
    // - attempting to prove the where clauses for impl2
    //
    // The last three steps are encapsulated in `fulfill_implication`.
    //
    // See RFC 1210 for more details and justification.

    // Currently we do not allow e.g. a negative impl to specialize a positive one
    if tcx.impl_polarity(impl1_def_id) != tcx.impl_polarity(impl2_def_id) {
        return false;
    }

    // create a parameter environment corresponding to a (skolemized) instantiation of impl1
    let penv = tcx.param_env(impl1_def_id);
    let impl1_trait_ref = tcx.impl_trait_ref(impl1_def_id).unwrap();

    // Create a infcx, taking the predicates of impl1 as assumptions:
    let result = tcx.infer_ctxt().enter(|infcx| {
        // Normalize the trait reference. The WF rules ought to ensure
        // that this always succeeds.
        let impl1_trait_ref =
            match traits::fully_normalize(&infcx,
                                          ObligationCause::dummy(),
                                          penv,
                                          &impl1_trait_ref) {
                Ok(impl1_trait_ref) => impl1_trait_ref,
                Err(err) => {
                    bug!("failed to fully normalize {:?}: {:?}", impl1_trait_ref, err);
                }
            };

        // Attempt to prove that impl2 applies, given all of the above.
        fulfill_implication(&infcx, penv, impl1_trait_ref, impl2_def_id).is_ok()
    });

    tcx.specializes_cache.borrow_mut().insert(impl1_def_id, impl2_def_id, result);
    result
}

/// Attempt to fulfill all obligations of `target_impl` after unification with
/// `source_trait_ref`. If successful, returns a substitution for *all* the
/// generics of `target_impl`, including both those needed to unify with
/// `source_trait_ref` and those whose identity is determined via a where
/// clause in the impl.
fn fulfill_implication<'a, 'gcx, 'tcx>(infcx: &InferCtxt<'a, 'gcx, 'tcx>,
                                       param_env: ty::ParamEnv<'tcx>,
                                       source_trait_ref: ty::TraitRef<'tcx>,
                                       target_impl: DefId)
                                       -> Result<&'tcx Substs<'tcx>, ()> {
    let selcx = &mut SelectionContext::new(&infcx);
    let target_substs = infcx.fresh_substs_for_item(DUMMY_SP, target_impl);
    let (target_trait_ref, mut obligations) = impl_trait_ref_and_oblig(selcx,
                                                                       param_env,
                                                                       target_impl,
                                                                       target_substs);

    // do the impls unify? If not, no specialization.
    match infcx.at(&ObligationCause::dummy(), param_env)
               .eq(source_trait_ref, target_trait_ref) {
        Ok(InferOk { obligations: o, .. }) => {
            obligations.extend(o);
        }
        Err(_) => {
            debug!("fulfill_implication: {:?} does not unify with {:?}",
                   source_trait_ref,
                   target_trait_ref);
            return Err(());
        }
    }

    // attempt to prove all of the predicates for impl2 given those for impl1
    // (which are packed up in penv)

    infcx.save_and_restore_in_snapshot_flag(|infcx| {
        let mut fulfill_cx = FulfillmentContext::new();
        for oblig in obligations.into_iter() {
            fulfill_cx.register_predicate_obligation(&infcx, oblig);
        }
        match fulfill_cx.select_all_or_error(infcx) {
            Err(errors) => {
                // no dice!
                debug!("fulfill_implication: for impls on {:?} and {:?}, \
                        could not fulfill: {:?} given {:?}",
                       source_trait_ref,
                       target_trait_ref,
                       errors,
                       param_env.caller_bounds);
                Err(())
            }

            Ok(()) => {
                debug!("fulfill_implication: an impl for {:?} specializes {:?}",
                       source_trait_ref,
                       target_trait_ref);

                // Now resolve the *substitution* we built for the target earlier, replacing
                // the inference variables inside with whatever we got from fulfillment.
                Ok(infcx.resolve_type_vars_if_possible(&target_substs))
            }
        }
    })
}

pub struct SpecializesCache {
    map: FxHashMap<(DefId, DefId), bool>,
}

impl SpecializesCache {
    pub fn new() -> Self {
        SpecializesCache {
            map: FxHashMap()
        }
    }

    pub fn check(&self, a: DefId, b: DefId) -> Option<bool> {
        self.map.get(&(a, b)).cloned()
    }

    pub fn insert(&mut self, a: DefId, b: DefId, result: bool) {
        self.map.insert((a, b), result);
    }
}

// Query provider for `specialization_graph_of`.
pub(super) fn specialization_graph_provider<'a, 'tcx>(tcx: TyCtxt<'a, 'tcx, 'tcx>,
                                                      trait_id: DefId)
                                                      -> Rc<specialization_graph::Graph> {
    let mut sg = specialization_graph::Graph::new();

    let mut trait_impls: Vec<DefId> = tcx.trait_impls_of(trait_id).iter().collect();

    // The coherence checking implementation seems to rely on impls being
    // iterated over (roughly) in definition order, so we are sorting by
    // negated CrateNum (so remote definitions are visited first) and then
    // by a flattened version of the DefIndex.
    trait_impls.sort_unstable_by_key(|def_id| {
        (-(def_id.krate.as_u32() as i64),
         def_id.index.address_space().index(),
         def_id.index.as_array_index())
    });

    for impl_def_id in trait_impls {
        if impl_def_id.is_local() {
            // This is where impl overlap checking happens:
            let insert_result = sg.insert(tcx, impl_def_id);
            // Report error if there was one.
            if let Err(overlap) = insert_result {
                let mut err = struct_span_err!(tcx.sess,
                                               tcx.span_of_impl(impl_def_id).unwrap(),
                                               E0119,
                                               "conflicting implementations of trait `{}`{}:",
                                               overlap.trait_desc,
                                               overlap.self_desc.clone().map_or(String::new(),
                                                                                |ty| {
                    format!(" for type `{}`", ty)
                }));

                match tcx.span_of_impl(overlap.with_impl) {
                    Ok(span) => {
                        err.span_label(span, format!("first implementation here"));
                        err.span_label(tcx.span_of_impl(impl_def_id).unwrap(),
                                       format!("conflicting implementation{}",
                                                overlap.self_desc
                                                    .map_or(String::new(),
                                                            |ty| format!(" for `{}`", ty))));
                    }
                    Err(cname) => {
                        err.note(&format!("conflicting implementation in crate `{}`", cname));
                    }
                }

                err.emit();
            }
        } else {
            let parent = tcx.impl_parent(impl_def_id).unwrap_or(trait_id);
            sg.record_impl_from_cstore(tcx, parent, impl_def_id)
        }
    }

    Rc::new(sg)
}
