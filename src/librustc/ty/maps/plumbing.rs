// Copyright 2012-2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The implementation of the query system itself. Defines the macros
//! that generate the actual methods on tcx which find and execute the
//! provider, manage the caches, and so forth.

use dep_graph::{DepNodeIndex, DepNode, DepKind, DepNodeColor};
use errors::DiagnosticBuilder;
use ty::{TyCtxt};
use ty::maps::Query; // NB: actually generated by the macros in this file
use ty::maps::config::QueryDescription;
use ty::item_path;

use rustc_data_structures::fx::{FxHashMap};
use std::cell::RefMut;
use std::marker::PhantomData;
use std::mem;
use syntax_pos::Span;

pub(super) struct QueryMap<'tcx, D: QueryDescription<'tcx>> {
    phantom: PhantomData<(D, &'tcx ())>,
    pub(super) map: FxHashMap<D::Key, QueryValue<D::Value>>,
}

pub(super) struct QueryValue<T> {
    pub(super) value: T,
    pub(super) index: DepNodeIndex,
}

impl<T> QueryValue<T> {
    pub(super) fn new(value: T,
                      dep_node_index: DepNodeIndex)
                      -> QueryValue<T> {
        QueryValue {
            value,
            index: dep_node_index,
        }
    }
}

impl<'tcx, M: QueryDescription<'tcx>> QueryMap<'tcx, M> {
    pub(super) fn new() -> QueryMap<'tcx, M> {
        QueryMap {
            phantom: PhantomData,
            map: FxHashMap(),
        }
    }
}

pub(super) struct CycleError<'a, 'tcx: 'a> {
    span: Span,
    cycle: RefMut<'a, [(Span, Query<'tcx>)]>,
}

impl<'a, 'gcx, 'tcx> TyCtxt<'a, 'gcx, 'tcx> {
    pub(super) fn report_cycle(self, CycleError { span, cycle }: CycleError)
        -> DiagnosticBuilder<'a>
    {
        // Subtle: release the refcell lock before invoking `describe()`
        // below by dropping `cycle`.
        let stack = cycle.to_vec();
        mem::drop(cycle);

        assert!(!stack.is_empty());

        // Disable naming impls with types in this path, since that
        // sometimes cycles itself, leading to extra cycle errors.
        // (And cycle errors around impls tend to occur during the
        // collect/coherence phases anyhow.)
        item_path::with_forced_impl_filename_line(|| {
            let mut err =
                struct_span_err!(self.sess, span, E0391,
                                 "unsupported cyclic reference between types/traits detected");
            err.span_label(span, "cyclic reference");

            err.span_note(stack[0].0, &format!("the cycle begins when {}...",
                                               stack[0].1.describe(self)));

            for &(span, ref query) in &stack[1..] {
                err.span_note(span, &format!("...which then requires {}...",
                                             query.describe(self)));
            }

            err.note(&format!("...which then again requires {}, completing the cycle.",
                              stack[0].1.describe(self)));

            return err
        })
    }

    pub(super) fn cycle_check<F, R>(self, span: Span, query: Query<'gcx>, compute: F)
                                    -> Result<R, CycleError<'a, 'gcx>>
        where F: FnOnce() -> R
    {
        {
            let mut stack = self.maps.query_stack.borrow_mut();
            if let Some((i, _)) = stack.iter().enumerate().rev()
                                       .find(|&(_, &(_, ref q))| *q == query) {
                return Err(CycleError {
                    span,
                    cycle: RefMut::map(stack, |stack| &mut stack[i..])
                });
            }
            stack.push((span, query));
        }

        let result = compute();

        self.maps.query_stack.borrow_mut().pop();

        Ok(result)
    }

    /// Try to read a node index for the node dep_node.
    /// A node will have an index, when it's already been marked green, or when we can mark it
    /// green. This function will mark the current task as a reader of the specified node, when
    /// the a node index can be found for that node.
    pub(super) fn try_mark_green_and_read(self, dep_node: &DepNode) -> Option<DepNodeIndex> {
        match self.dep_graph.node_color(dep_node) {
            Some(DepNodeColor::Green(dep_node_index)) => {
                self.dep_graph.read_index(dep_node_index);
                Some(dep_node_index)
            }
            Some(DepNodeColor::Red) => {
                None
            }
            None => {
                // try_mark_green (called below) will panic when full incremental
                // compilation is disabled. If that's the case, we can't try to mark nodes
                // as green anyway, so we can safely return None here.
                if !self.dep_graph.is_fully_enabled() {
                    return None;
                }
                match self.dep_graph.try_mark_green(self, &dep_node) {
                    Some(dep_node_index) => {
                        debug_assert!(self.dep_graph.is_green(dep_node_index));
                        self.dep_graph.read_index(dep_node_index);
                        Some(dep_node_index)
                    }
                    None => {
                        None
                    }
                }
            }
        }
    }
}

// If enabled, send a message to the profile-queries thread
macro_rules! profq_msg {
    ($tcx:expr, $msg:expr) => {
        if cfg!(debug_assertions) {
            if  $tcx.sess.profile_queries() {
                profq_msg($msg)
            }
        }
    }
}

// If enabled, format a key using its debug string, which can be
// expensive to compute (in terms of time).
macro_rules! profq_key {
    ($tcx:expr, $key:expr) => {
        if cfg!(debug_assertions) {
            if $tcx.sess.profile_queries_and_keys() {
                Some(format!("{:?}", $key))
            } else { None }
        } else { None }
    }
}

macro_rules! define_maps {
    (<$tcx:tt>
     $($(#[$attr:meta])*
       [$($modifiers:tt)*] fn $name:ident: $node:ident($K:ty) -> $V:ty,)*) => {

        use dep_graph::DepNodeIndex;
        use std::cell::RefCell;

        define_map_struct! {
            tcx: $tcx,
            input: ($(([$($modifiers)*] [$($attr)*] [$name]))*)
        }

        impl<$tcx> Maps<$tcx> {
            pub fn new(providers: IndexVec<CrateNum, Providers<$tcx>>)
                       -> Self {
                Maps {
                    providers,
                    query_stack: RefCell::new(vec![]),
                    $($name: RefCell::new(QueryMap::new())),*
                }
            }
        }

        #[allow(bad_style)]
        #[derive(Copy, Clone, Debug, PartialEq, Eq)]
        pub enum Query<$tcx> {
            $($(#[$attr])* $name($K)),*
        }

        #[allow(bad_style)]
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum QueryMsg {
            $($name(Option<String>)),*
        }

        impl<$tcx> Query<$tcx> {
            pub fn describe(&self, tcx: TyCtxt) -> String {
                let (r, name) = match *self {
                    $(Query::$name(key) => {
                        (queries::$name::describe(tcx, key), stringify!($name))
                    })*
                };
                if tcx.sess.verbose() {
                    format!("{} [{}]", r, name)
                } else {
                    r
                }
            }
        }

        pub mod queries {
            use std::marker::PhantomData;

            $(#[allow(bad_style)]
            pub struct $name<$tcx> {
                data: PhantomData<&$tcx ()>
            })*
        }

        $(impl<$tcx> QueryConfig for queries::$name<$tcx> {
            type Key = $K;
            type Value = $V;
        }

        impl<'a, $tcx, 'lcx> queries::$name<$tcx> {

            #[allow(unused)]
            fn to_dep_node(tcx: TyCtxt<'a, $tcx, 'lcx>, key: &$K) -> DepNode {
                use dep_graph::DepConstructor::*;

                DepNode::new(tcx, $node(*key))
            }

            fn try_get_with(tcx: TyCtxt<'a, $tcx, 'lcx>,
                            mut span: Span,
                            key: $K)
                            -> Result<$V, CycleError<'a, $tcx>>
            {
                debug!("ty::queries::{}::try_get_with(key={:?}, span={:?})",
                       stringify!($name),
                       key,
                       span);

                profq_msg!(tcx,
                    ProfileQueriesMsg::QueryBegin(
                        span.data(),
                        QueryMsg::$name(profq_key!(tcx, key))
                    )
                );

                if let Some(value) = tcx.maps.$name.borrow().map.get(&key) {
                    profq_msg!(tcx, ProfileQueriesMsg::CacheHit);
                    tcx.dep_graph.read_index(value.index);
                    return Ok((&value.value).clone());
                }

                // FIXME(eddyb) Get more valid Span's on queries.
                // def_span guard is necessary to prevent a recursive loop,
                // default_span calls def_span query internally.
                if span == DUMMY_SP && stringify!($name) != "def_span" {
                    span = key.default_span(tcx)
                }

                // Fast path for when incr. comp. is off. `to_dep_node` is
                // expensive for some DepKinds.
                if !tcx.dep_graph.is_fully_enabled() {
                    let null_dep_node = DepNode::new_no_params(::dep_graph::DepKind::Null);
                    return Self::force(tcx, key, span, null_dep_node)
                                .map(|(v, _)| v);
                }

                let dep_node = Self::to_dep_node(tcx, &key);

                if dep_node.kind.is_anon() {
                    profq_msg!(tcx, ProfileQueriesMsg::ProviderBegin);

                    let res = tcx.cycle_check(span, Query::$name(key), || {
                        tcx.sess.diagnostic().track_diagnostics(|| {
                            tcx.dep_graph.with_anon_task(dep_node.kind, || {
                                Self::compute_result(tcx.global_tcx(), key)
                            })
                        })
                    })?;

                    profq_msg!(tcx, ProfileQueriesMsg::ProviderEnd);
                    let ((result, dep_node_index), diagnostics) = res;

                    tcx.dep_graph.read_index(dep_node_index);

                    tcx.on_disk_query_result_cache
                       .store_diagnostics_for_anon_node(dep_node_index, diagnostics);

                    let value = QueryValue::new(result, dep_node_index);

                    return Ok((&tcx.maps
                                    .$name
                                    .borrow_mut()
                                    .map
                                    .entry(key)
                                    .or_insert(value)
                                    .value).clone());
                }

                if !dep_node.kind.is_input() {
                    if let Some(dep_node_index) = tcx.try_mark_green_and_read(&dep_node) {
                        profq_msg!(tcx, ProfileQueriesMsg::CacheHit);
                        return Self::load_from_disk_and_cache_in_memory(tcx,
                                                                        key,
                                                                        span,
                                                                        dep_node_index,
                                                                        &dep_node)
                    }
                }

                match Self::force(tcx, key, span, dep_node) {
                    Ok((result, dep_node_index)) => {
                        tcx.dep_graph.read_index(dep_node_index);
                        Ok(result)
                    }
                    Err(e) => Err(e)
                }
            }

            /// Ensure that either this query has all green inputs or been executed.
            /// Executing query::ensure(D) is considered a read of the dep-node D.
            ///
            /// This function is particularly useful when executing passes for their
            /// side-effects -- e.g., in order to report errors for erroneous programs.
            ///
            /// Note: The optimization is only available during incr. comp.
            pub fn ensure(tcx: TyCtxt<'a, $tcx, 'lcx>, key: $K) -> () {
                let dep_node = Self::to_dep_node(tcx, &key);

                // Ensuring an "input" or anonymous query makes no sense
                assert!(!dep_node.kind.is_anon());
                assert!(!dep_node.kind.is_input());
                if tcx.try_mark_green_and_read(&dep_node).is_none() {
                    // A None return from `try_mark_green_and_read` means that this is either
                    // a new dep node or that the dep node has already been marked red.
                    // Either way, we can't call `dep_graph.read()` as we don't have the
                    // DepNodeIndex. We must invoke the query itself. The performance cost
                    // this introduces should be negligible as we'll immediately hit the
                    // in-memory cache, or another query down the line will.
                    let _ = tcx.$name(key);
                }
            }

            fn compute_result(tcx: TyCtxt<'a, $tcx, 'lcx>, key: $K) -> $V {
                let provider = tcx.maps.providers[key.map_crate()].$name;
                provider(tcx.global_tcx(), key)
            }

            fn load_from_disk_and_cache_in_memory(tcx: TyCtxt<'a, $tcx, 'lcx>,
                                                  key: $K,
                                                  span: Span,
                                                  dep_node_index: DepNodeIndex,
                                                  dep_node: &DepNode)
                                                  -> Result<$V, CycleError<'a, $tcx>>
            {
                debug_assert!(tcx.dep_graph.is_green(dep_node_index));

                let result = if tcx.sess.opts.debugging_opts.incremental_queries &&
                                Self::cache_on_disk(key) {
                    let prev_dep_node_index =
                        tcx.dep_graph.prev_dep_node_index_of(dep_node);
                    Self::load_from_disk(tcx.global_tcx(), prev_dep_node_index)
                } else {
                    let (result, _ ) = tcx.cycle_check(span, Query::$name(key), || {
                        // The diagnostics for this query have already been
                        // promoted to the current session during
                        // try_mark_green(), so we can ignore them here.
                        tcx.sess.diagnostic().track_diagnostics(|| {
                            // The dep-graph for this computation is already in
                            // place
                            tcx.dep_graph.with_ignore(|| {
                                Self::compute_result(tcx, key)
                            })
                        })
                    })?;
                    result
                };

                // If -Zincremental-verify-ich is specified, re-hash results from
                // the cache and make sure that they have the expected fingerprint.
                if tcx.sess.opts.debugging_opts.incremental_verify_ich {
                    use rustc_data_structures::stable_hasher::{StableHasher, HashStable};
                    use ich::Fingerprint;

                    assert!(Some(tcx.dep_graph.fingerprint_of(dep_node)) ==
                            tcx.dep_graph.prev_fingerprint_of(dep_node),
                            "Fingerprint for green query instance not loaded \
                             from cache: {:?}", dep_node);

                    debug!("BEGIN verify_ich({:?})", dep_node);
                    let mut hcx = tcx.create_stable_hashing_context();
                    let mut hasher = StableHasher::new();

                    result.hash_stable(&mut hcx, &mut hasher);

                    let new_hash: Fingerprint = hasher.finish();
                    debug!("END verify_ich({:?})", dep_node);

                    let old_hash = tcx.dep_graph.fingerprint_of(dep_node);

                    assert!(new_hash == old_hash, "Found unstable fingerprints \
                        for {:?}", dep_node);
                }

                if tcx.sess.opts.debugging_opts.query_dep_graph {
                    tcx.dep_graph.mark_loaded_from_cache(dep_node_index, true);
                }

                let value = QueryValue::new(result, dep_node_index);

                Ok((&tcx.maps
                         .$name
                         .borrow_mut()
                         .map
                         .entry(key)
                         .or_insert(value)
                         .value).clone())
            }

            fn force(tcx: TyCtxt<'a, $tcx, 'lcx>,
                     key: $K,
                     span: Span,
                     dep_node: DepNode)
                     -> Result<($V, DepNodeIndex), CycleError<'a, $tcx>> {
                debug_assert!(tcx.dep_graph.node_color(&dep_node).is_none());

                profq_msg!(tcx, ProfileQueriesMsg::ProviderBegin);
                let res = tcx.cycle_check(span, Query::$name(key), || {
                    tcx.sess.diagnostic().track_diagnostics(|| {
                        if dep_node.kind.is_eval_always() {
                            tcx.dep_graph.with_eval_always_task(dep_node,
                                                                tcx,
                                                                key,
                                                                Self::compute_result)
                        } else {
                            tcx.dep_graph.with_task(dep_node,
                                                    tcx,
                                                    key,
                                                    Self::compute_result)
                        }
                    })
                })?;
                profq_msg!(tcx, ProfileQueriesMsg::ProviderEnd);

                let ((result, dep_node_index), diagnostics) = res;

                if tcx.sess.opts.debugging_opts.query_dep_graph {
                    tcx.dep_graph.mark_loaded_from_cache(dep_node_index, false);
                }

                if dep_node.kind != ::dep_graph::DepKind::Null {
                    tcx.on_disk_query_result_cache
                       .store_diagnostics(dep_node_index, diagnostics);
                }

                let value = QueryValue::new(result, dep_node_index);

                Ok(((&tcx.maps
                         .$name
                         .borrow_mut()
                         .map
                         .entry(key)
                         .or_insert(value)
                         .value).clone(),
                   dep_node_index))
            }

            pub fn try_get(tcx: TyCtxt<'a, $tcx, 'lcx>, span: Span, key: $K)
                           -> Result<$V, DiagnosticBuilder<'a>> {
                match Self::try_get_with(tcx, span, key) {
                    Ok(e) => Ok(e),
                    Err(e) => Err(tcx.report_cycle(e)),
                }
            }
        })*

        #[derive(Copy, Clone)]
        pub struct TyCtxtAt<'a, 'gcx: 'a+'tcx, 'tcx: 'a> {
            pub tcx: TyCtxt<'a, 'gcx, 'tcx>,
            pub span: Span,
        }

        impl<'a, 'gcx, 'tcx> Deref for TyCtxtAt<'a, 'gcx, 'tcx> {
            type Target = TyCtxt<'a, 'gcx, 'tcx>;
            fn deref(&self) -> &Self::Target {
                &self.tcx
            }
        }

        impl<'a, $tcx, 'lcx> TyCtxt<'a, $tcx, 'lcx> {
            /// Return a transparent wrapper for `TyCtxt` which uses
            /// `span` as the location of queries performed through it.
            pub fn at(self, span: Span) -> TyCtxtAt<'a, $tcx, 'lcx> {
                TyCtxtAt {
                    tcx: self,
                    span
                }
            }

            $($(#[$attr])*
            pub fn $name(self, key: $K) -> $V {
                self.at(DUMMY_SP).$name(key)
            })*
        }

        impl<'a, $tcx, 'lcx> TyCtxtAt<'a, $tcx, 'lcx> {
            $($(#[$attr])*
            pub fn $name(self, key: $K) -> $V {
                queries::$name::try_get(self.tcx, self.span, key).unwrap_or_else(|mut e| {
                    e.emit();
                    Value::from_cycle_error(self.global_tcx())
                })
            })*
        }

        define_provider_struct! {
            tcx: $tcx,
            input: ($(([$($modifiers)*] [$name] [$K] [$V]))*)
        }

        impl<$tcx> Copy for Providers<$tcx> {}
        impl<$tcx> Clone for Providers<$tcx> {
            fn clone(&self) -> Self { *self }
        }
    }
}

macro_rules! define_map_struct {
    (tcx: $tcx:tt,
     input: ($(([$(modifiers:tt)*] [$($attr:tt)*] [$name:ident]))*)) => {
        pub struct Maps<$tcx> {
            providers: IndexVec<CrateNum, Providers<$tcx>>,
            query_stack: RefCell<Vec<(Span, Query<$tcx>)>>,
            $($(#[$attr])*  $name: RefCell<QueryMap<$tcx, queries::$name<$tcx>>>,)*
        }
    };
}

macro_rules! define_provider_struct {
    (tcx: $tcx:tt,
     input: ($(([$($modifiers:tt)*] [$name:ident] [$K:ty] [$R:ty]))*)) => {
        pub struct Providers<$tcx> {
            $(pub $name: for<'a> fn(TyCtxt<'a, $tcx, $tcx>, $K) -> $R,)*
        }

        impl<$tcx> Default for Providers<$tcx> {
            fn default() -> Self {
                $(fn $name<'a, $tcx>(_: TyCtxt<'a, $tcx, $tcx>, key: $K) -> $R {
                    bug!("tcx.maps.{}({:?}) unsupported by its crate",
                         stringify!($name), key);
                })*
                Providers { $($name),* }
            }
        }
    };
}


/// The red/green evaluation system will try to mark a specific DepNode in the
/// dependency graph as green by recursively trying to mark the dependencies of
/// that DepNode as green. While doing so, it will sometimes encounter a DepNode
/// where we don't know if it is red or green and we therefore actually have
/// to recompute its value in order to find out. Since the only piece of
/// information that we have at that point is the DepNode we are trying to
/// re-evaluate, we need some way to re-run a query from just that. This is what
/// `force_from_dep_node()` implements.
///
/// In the general case, a DepNode consists of a DepKind and an opaque
/// GUID/fingerprint that will uniquely identify the node. This GUID/fingerprint
/// is usually constructed by computing a stable hash of the query-key that the
/// DepNode corresponds to. Consequently, it is not in general possible to go
/// back from hash to query-key (since hash functions are not reversible). For
/// this reason `force_from_dep_node()` is expected to fail from time to time
/// because we just cannot find out, from the DepNode alone, what the
/// corresponding query-key is and therefore cannot re-run the query.
///
/// The system deals with this case letting `try_mark_green` fail which forces
/// the root query to be re-evaluated.
///
/// Now, if force_from_dep_node() would always fail, it would be pretty useless.
/// Fortunately, we can use some contextual information that will allow us to
/// reconstruct query-keys for certain kinds of DepNodes. In particular, we
/// enforce by construction that the GUID/fingerprint of certain DepNodes is a
/// valid DefPathHash. Since we also always build a huge table that maps every
/// DefPathHash in the current codebase to the corresponding DefId, we have
/// everything we need to re-run the query.
///
/// Take the `mir_validated` query as an example. Like many other queries, it
/// just has a single parameter: the DefId of the item it will compute the
/// validated MIR for. Now, when we call `force_from_dep_node()` on a dep-node
/// with kind `MirValidated`, we know that the GUID/fingerprint of the dep-node
/// is actually a DefPathHash, and can therefore just look up the corresponding
/// DefId in `tcx.def_path_hash_to_def_id`.
///
/// When you implement a new query, it will likely have a corresponding new
/// DepKind, and you'll have to support it here in `force_from_dep_node()`. As
/// a rule of thumb, if your query takes a DefId or DefIndex as sole parameter,
/// then `force_from_dep_node()` should not fail for it. Otherwise, you can just
/// add it to the "We don't have enough information to reconstruct..." group in
/// the match below.
pub fn force_from_dep_node<'a, 'gcx, 'lcx>(tcx: TyCtxt<'a, 'gcx, 'lcx>,
                                           dep_node: &DepNode)
                                           -> bool {
    use ty::maps::keys::Key;
    use hir::def_id::LOCAL_CRATE;

    // We must avoid ever having to call force_from_dep_node() for a
    // DepNode::CodegenUnit:
    // Since we cannot reconstruct the query key of a DepNode::CodegenUnit, we
    // would always end up having to evaluate the first caller of the
    // `codegen_unit` query that *is* reconstructible. This might very well be
    // the `compile_codegen_unit` query, thus re-translating the whole CGU just
    // to re-trigger calling the `codegen_unit` query with the right key. At
    // that point we would already have re-done all the work we are trying to
    // avoid doing in the first place.
    // The solution is simple: Just explicitly call the `codegen_unit` query for
    // each CGU, right after partitioning. This way `try_mark_green` will always
    // hit the cache instead of having to go through `force_from_dep_node`.
    // This assertion makes sure, we actually keep applying the solution above.
    debug_assert!(dep_node.kind != DepKind::CodegenUnit,
                  "calling force_from_dep_node() on DepKind::CodegenUnit");

    if !dep_node.kind.can_reconstruct_query_key() {
        return false
    }

    macro_rules! def_id {
        () => {
            if let Some(def_id) = dep_node.extract_def_id(tcx) {
                def_id
            } else {
                // return from the whole function
                return false
            }
        }
    };

    macro_rules! krate {
        () => { (def_id!()).krate }
    };

    macro_rules! force {
        ($query:ident, $key:expr) => {
            {
                use $crate::util::common::{ProfileQueriesMsg, profq_msg};

                // FIXME(eddyb) Get more valid Span's on queries.
                // def_span guard is necessary to prevent a recursive loop,
                // default_span calls def_span query internally.
                let span = if stringify!($query) != "def_span" {
                    $key.default_span(tcx)
                } else {
                    ::syntax_pos::DUMMY_SP
                };

                profq_msg!(tcx,
                    ProfileQueriesMsg::QueryBegin(
                        span.data(),
                        ::ty::maps::QueryMsg::$query(profq_key!(tcx, $key))
                    )
                );

                match ::ty::maps::queries::$query::force(tcx, $key, span, *dep_node) {
                    Ok(_) => {},
                    Err(e) => {
                        tcx.report_cycle(e).emit();
                    }
                }
            }
        }
    };

    // FIXME(#45015): We should try move this boilerplate code into a macro
    //                somehow.
    match dep_node.kind {
        // These are inputs that are expected to be pre-allocated and that
        // should therefore always be red or green already
        DepKind::AllLocalTraitImpls |
        DepKind::Krate |
        DepKind::CrateMetadata |
        DepKind::HirBody |
        DepKind::Hir |

        // This are anonymous nodes
        DepKind::TraitSelect |

        // We don't have enough information to reconstruct the query key of
        // these
        DepKind::IsCopy |
        DepKind::IsSized |
        DepKind::IsFreeze |
        DepKind::NeedsDrop |
        DepKind::Layout |
        DepKind::ConstEval |
        DepKind::InstanceSymbolName |
        DepKind::MirShim |
        DepKind::BorrowCheckKrate |
        DepKind::Specializes |
        DepKind::ImplementationsOfTrait |
        DepKind::TypeParamPredicates |
        DepKind::CodegenUnit |
        DepKind::CompileCodegenUnit |
        DepKind::FulfillObligation |
        DepKind::VtableMethods |
        DepKind::EraseRegionsTy |
        DepKind::NormalizeTy |

        // This one should never occur in this context
        DepKind::Null => {
            bug!("force_from_dep_node() - Encountered {:?}", dep_node)
        }

        // These are not queries
        DepKind::CoherenceCheckTrait |
        DepKind::ItemVarianceConstraints => {
            return false
        }

        DepKind::RegionScopeTree => { force!(region_scope_tree, def_id!()); }

        DepKind::Coherence => { force!(crate_inherent_impls, LOCAL_CRATE); }
        DepKind::CoherenceInherentImplOverlapCheck => {
            force!(crate_inherent_impls_overlap_check, LOCAL_CRATE)
        },
        DepKind::PrivacyAccessLevels => { force!(privacy_access_levels, LOCAL_CRATE); }
        DepKind::MirBuilt => { force!(mir_built, def_id!()); }
        DepKind::MirConstQualif => { force!(mir_const_qualif, def_id!()); }
        DepKind::MirConst => { force!(mir_const, def_id!()); }
        DepKind::MirValidated => { force!(mir_validated, def_id!()); }
        DepKind::MirOptimized => { force!(optimized_mir, def_id!()); }

        DepKind::BorrowCheck => { force!(borrowck, def_id!()); }
        DepKind::MirBorrowCheck => { force!(mir_borrowck, def_id!()); }
        DepKind::UnsafetyCheckResult => { force!(unsafety_check_result, def_id!()); }
        DepKind::Reachability => { force!(reachable_set, LOCAL_CRATE); }
        DepKind::MirKeys => { force!(mir_keys, LOCAL_CRATE); }
        DepKind::CrateVariances => { force!(crate_variances, LOCAL_CRATE); }
        DepKind::AssociatedItems => { force!(associated_item, def_id!()); }
        DepKind::TypeOfItem => { force!(type_of, def_id!()); }
        DepKind::GenericsOfItem => { force!(generics_of, def_id!()); }
        DepKind::PredicatesOfItem => { force!(predicates_of, def_id!()); }
        DepKind::InferredOutlivesOf => { force!(inferred_outlives_of, def_id!()); }
        DepKind::SuperPredicatesOfItem => { force!(super_predicates_of, def_id!()); }
        DepKind::TraitDefOfItem => { force!(trait_def, def_id!()); }
        DepKind::AdtDefOfItem => { force!(adt_def, def_id!()); }
        DepKind::IsAutoImpl => { force!(is_auto_impl, def_id!()); }
        DepKind::ImplTraitRef => { force!(impl_trait_ref, def_id!()); }
        DepKind::ImplPolarity => { force!(impl_polarity, def_id!()); }
        DepKind::ClosureKind => { force!(closure_kind, def_id!()); }
        DepKind::FnSignature => { force!(fn_sig, def_id!()); }
        DepKind::GenSignature => { force!(generator_sig, def_id!()); }
        DepKind::CoerceUnsizedInfo => { force!(coerce_unsized_info, def_id!()); }
        DepKind::ItemVariances => { force!(variances_of, def_id!()); }
        DepKind::IsConstFn => { force!(is_const_fn, def_id!()); }
        DepKind::IsForeignItem => { force!(is_foreign_item, def_id!()); }
        DepKind::SizedConstraint => { force!(adt_sized_constraint, def_id!()); }
        DepKind::DtorckConstraint => { force!(adt_dtorck_constraint, def_id!()); }
        DepKind::AdtDestructor => { force!(adt_destructor, def_id!()); }
        DepKind::AssociatedItemDefIds => { force!(associated_item_def_ids, def_id!()); }
        DepKind::InherentImpls => { force!(inherent_impls, def_id!()); }
        DepKind::TypeckBodiesKrate => { force!(typeck_item_bodies, LOCAL_CRATE); }
        DepKind::TypeckTables => { force!(typeck_tables_of, def_id!()); }
        DepKind::UsedTraitImports => { force!(used_trait_imports, def_id!()); }
        DepKind::HasTypeckTables => { force!(has_typeck_tables, def_id!()); }
        DepKind::SymbolName => { force!(def_symbol_name, def_id!()); }
        DepKind::SpecializationGraph => { force!(specialization_graph_of, def_id!()); }
        DepKind::ObjectSafety => { force!(is_object_safe, def_id!()); }
        DepKind::TraitImpls => { force!(trait_impls_of, def_id!()); }

        DepKind::ParamEnv => { force!(param_env, def_id!()); }
        DepKind::DescribeDef => { force!(describe_def, def_id!()); }
        DepKind::DefSpan => { force!(def_span, def_id!()); }
        DepKind::LookupStability => { force!(lookup_stability, def_id!()); }
        DepKind::LookupDeprecationEntry => {
            force!(lookup_deprecation_entry, def_id!());
        }
        DepKind::ItemBodyNestedBodies => { force!(item_body_nested_bodies, def_id!()); }
        DepKind::ConstIsRvaluePromotableToStatic => {
            force!(const_is_rvalue_promotable_to_static, def_id!());
        }
        DepKind::RvaluePromotableMap => { force!(rvalue_promotable_map, def_id!()); }
        DepKind::ImplParent => { force!(impl_parent, def_id!()); }
        DepKind::TraitOfItem => { force!(trait_of_item, def_id!()); }
        DepKind::IsExportedSymbol => { force!(is_exported_symbol, def_id!()); }
        DepKind::IsMirAvailable => { force!(is_mir_available, def_id!()); }
        DepKind::ItemAttrs => { force!(item_attrs, def_id!()); }
        DepKind::FnArgNames => { force!(fn_arg_names, def_id!()); }
        DepKind::DylibDepFormats => { force!(dylib_dependency_formats, krate!()); }
        DepKind::IsPanicRuntime => { force!(is_panic_runtime, krate!()); }
        DepKind::IsCompilerBuiltins => { force!(is_compiler_builtins, krate!()); }
        DepKind::HasGlobalAllocator => { force!(has_global_allocator, krate!()); }
        DepKind::ExternCrate => { force!(extern_crate, def_id!()); }
        DepKind::LintLevels => { force!(lint_levels, LOCAL_CRATE); }
        DepKind::InScopeTraits => { force!(in_scope_traits_map, def_id!().index); }
        DepKind::ModuleExports => { force!(module_exports, def_id!()); }
        DepKind::IsSanitizerRuntime => { force!(is_sanitizer_runtime, krate!()); }
        DepKind::IsProfilerRuntime => { force!(is_profiler_runtime, krate!()); }
        DepKind::GetPanicStrategy => { force!(panic_strategy, krate!()); }
        DepKind::IsNoBuiltins => { force!(is_no_builtins, krate!()); }
        DepKind::ImplDefaultness => { force!(impl_defaultness, def_id!()); }
        DepKind::ExportedSymbolIds => { force!(exported_symbol_ids, krate!()); }
        DepKind::NativeLibraries => { force!(native_libraries, krate!()); }
        DepKind::PluginRegistrarFn => { force!(plugin_registrar_fn, krate!()); }
        DepKind::DeriveRegistrarFn => { force!(derive_registrar_fn, krate!()); }
        DepKind::CrateDisambiguator => { force!(crate_disambiguator, krate!()); }
        DepKind::CrateHash => { force!(crate_hash, krate!()); }
        DepKind::OriginalCrateName => { force!(original_crate_name, krate!()); }

        DepKind::AllTraitImplementations => {
            force!(all_trait_implementations, krate!());
        }

        DepKind::IsDllimportForeignItem => {
            force!(is_dllimport_foreign_item, def_id!());
        }
        DepKind::IsStaticallyIncludedForeignItem => {
            force!(is_statically_included_foreign_item, def_id!());
        }
        DepKind::NativeLibraryKind => { force!(native_library_kind, def_id!()); }
        DepKind::LinkArgs => { force!(link_args, LOCAL_CRATE); }

        DepKind::NamedRegion => { force!(named_region_map, def_id!().index); }
        DepKind::IsLateBound => { force!(is_late_bound_map, def_id!().index); }
        DepKind::ObjectLifetimeDefaults => {
            force!(object_lifetime_defaults_map, def_id!().index);
        }

        DepKind::Visibility => { force!(visibility, def_id!()); }
        DepKind::DepKind => { force!(dep_kind, krate!()); }
        DepKind::CrateName => { force!(crate_name, krate!()); }
        DepKind::ItemChildren => { force!(item_children, def_id!()); }
        DepKind::ExternModStmtCnum => { force!(extern_mod_stmt_cnum, def_id!()); }
        DepKind::GetLangItems => { force!(get_lang_items, LOCAL_CRATE); }
        DepKind::DefinedLangItems => { force!(defined_lang_items, krate!()); }
        DepKind::MissingLangItems => { force!(missing_lang_items, krate!()); }
        DepKind::ExternConstBody => { force!(extern_const_body, def_id!()); }
        DepKind::VisibleParentMap => { force!(visible_parent_map, LOCAL_CRATE); }
        DepKind::MissingExternCrateItem => {
            force!(missing_extern_crate_item, krate!());
        }
        DepKind::UsedCrateSource => { force!(used_crate_source, krate!()); }
        DepKind::PostorderCnums => { force!(postorder_cnums, LOCAL_CRATE); }
        DepKind::HasCloneClosures => { force!(has_clone_closures, krate!()); }
        DepKind::HasCopyClosures => { force!(has_copy_closures, krate!()); }

        DepKind::Freevars => { force!(freevars, def_id!()); }
        DepKind::MaybeUnusedTraitImport => {
            force!(maybe_unused_trait_import, def_id!());
        }
        DepKind::MaybeUnusedExternCrates => { force!(maybe_unused_extern_crates, LOCAL_CRATE); }
        DepKind::StabilityIndex => { force!(stability_index, LOCAL_CRATE); }
        DepKind::AllCrateNums => { force!(all_crate_nums, LOCAL_CRATE); }
        DepKind::ExportedSymbols => { force!(exported_symbols, krate!()); }
        DepKind::CollectAndPartitionTranslationItems => {
            force!(collect_and_partition_translation_items, LOCAL_CRATE);
        }
        DepKind::ExportName => { force!(export_name, def_id!()); }
        DepKind::ContainsExternIndicator => {
            force!(contains_extern_indicator, def_id!());
        }
        DepKind::IsTranslatedFunction => { force!(is_translated_function, def_id!()); }
        DepKind::OutputFilenames => { force!(output_filenames, LOCAL_CRATE); }
    }

    true
}
