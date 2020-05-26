//! Propagates assignment destinations backwards in the CFG to eliminate redundant assignments.
//!
//! # Motivation
//!
//! MIR building can insert a lot of redundant copies, and Rust code in general often tends to move
//! values around a lot. The result is a lot of assignments of the form `dest = {move} src;` in MIR.
//! MIR building for constants in particular tends to create additional locals that are only used
//! inside a single block to shuffle a value around unnecessarily.
//!
//! LLVM by itself is not good enough at eliminating these redundant copies (eg. see
//! https://github.com/rust-lang/rust/issues/32966), so this leaves some performance on the table
//! that we can regain by implementing an optimization for removing these assign statements in rustc
//! itself. When this optimization runs fast enough, it can also speed up the constant evaluation
//! and code generation phases of rustc due to the reduced number of statements and locals.
//!
//! # The Optimization
//!
//! Conceptually, this optimization is "destination propagation". It is similar to the Named Return
//! Value Optimization, or NRVO, known from the C++ world, except that it isn't limited to return
//! values or the return place `_0`. On a very high level, independent of the actual implementation
//! details, it does the following:
//!
//! 1) Identify `dest = src;` statements that can be soundly eliminated.
//! 2) Replace all mentions of `src` with `dest` ("unifying" them and propagating the destination
//!    backwards).
//! 3) Delete the `dest = src;` statement (by making it a `nop`).
//!
//! Step 1) is by far the hardest, so it is explained in more detail below.
//!
//! ## Soundness
//!
//! Given an `Assign` statement `dest = src;`, where `dest` is a `Place` and `src` is an `Rvalue`,
//! there are a few requirements that must hold for the optimization to be sound:
//!
//! * `dest` must not contain any *indirection* through a pointer. It must access part of the base
//!   local. Otherwise it might point to arbitrary memory that is hard to track.
//!
//!   It must also not contain any indexing projections, since those take an arbitrary `Local` as
//!   the index, and that local might only be initialized shortly before `dest` is used.
//!
//!   Subtle case: If `dest` is a, or projects through a union, then we have to make sure that there
//!   remains an assignment to it, since that sets the "active field" of the union. But if `src` is
//!   a ZST, it might not be initialized, so there might not be any use of it before the assignment,
//!   and performing the optimization would simply delete the assignment, leaving `dest`
//!   uninitialized.
//!
//! * `src` must be a bare `Local` without any indirections or field projections (FIXME: Why?).
//!   It can be copied or moved by the assignment.
//!
//! * The `dest` and `src` locals must never be [*live*][liveness] at the same time. If they are, it
//!   means that they both hold a (potentially different) value that is needed by a future use of
//!   the locals. Unifying them would overwrite one of the values.
//!
//!   Note that computing liveness of locals that have had their address taken is more difficult:
//!   Short of doing full escape analysis on the address/pointer/reference, the pass would need to
//!   assume that any operation that can potentially involve opaque user code (such as function
//!   calls, destructors, and inline assembly) may access any local that had its address taken
//!   before that point.
//!
//! Here, the first two conditions are simple structural requirements on the `Assign` statements
//! that can be trivially checked. The liveness requirement however is more difficult and costly to
//! check.
//!
//! ## Previous Work
//!
//! A [previous attempt] at implementing an optimization like this turned out to be a significant
//! regression in compiler performance. Fixing the regressions introduced a lot of undesirable
//! complexity to the implementation.
//!
//! A [subsequent approach] tried to avoid the costly computation by limiting itself to acyclic
//! CFGs, but still turned out to be far too costly to run due to suboptimal performance within
//! individual basic blocks, requiring a walk across the entire block for every assignment found
//! within the block. For the `tuple-stress` benchmark, which has 458745 statements in a single
//! block, this proved to be far too costly.
//!
//! Since the first attempt at this, the compiler has improved dramatically, and new analysis
//! frameworks have been added that should make this approach viable without requiring a limited
//! approach that only works for some classes of CFGs:
//! - rustc now has a powerful dataflow analysis framework that can handle forwards and backwards
//!   analyses efficiently.
//! - Layout optimizations for generators have been added to improve code generation for
//!   async/await, which are very similar in spirit to what this optimization does. Both walk the
//!   MIR and record conflicting uses of locals in a `BitMatrix`.
//!
//! Also, rustc now has a simple NRVO pass (see `nrvo.rs`), which handles a subset of the cases that
//! this destination propagation pass handles, proving that similar optimizations can be performed
//! on MIR.
//!
//! ## Pre/Post Optimization
//!
//! It is recommended to run `SimplifyCfg` and then `SimplifyLocals` some time after this pass, as
//! it replaces the eliminated assign statements with `nop`s and leaves unused locals behind.
//!
//! [liveness]: https://en.wikipedia.org/wiki/Live_variable_analysis
//! [previous attempt]: https://github.com/rust-lang/rust/pull/47954
//! [subsequent approach]: https://github.com/rust-lang/rust/pull/71003

use crate::dataflow::{self, Analysis};
use crate::{
    transform::{MirPass, MirSource},
    util::{dump_mir, PassWhere},
};
use dataflow::impls::{MaybeInitializedLocals, MaybeLiveLocals};
use rustc_data_structures::unify::{InPlaceUnificationTable, UnifyKey};
use rustc_index::{
    bit_set::{BitMatrix, BitSet},
    vec::IndexVec,
};
use rustc_middle::mir::tcx::PlaceTy;
use rustc_middle::mir::visit::{MutVisitor, PlaceContext, Visitor};
use rustc_middle::mir::{
    traversal, Body, Local, LocalKind, Location, Operand, Place, PlaceElem, Rvalue, Statement,
    StatementKind, Terminator, TerminatorKind,
};
use rustc_middle::ty::{self, Ty, TyCtxt};

const MAX_LOCALS: usize = 500;

pub struct DestinationPropagation;

impl<'tcx> MirPass<'tcx> for DestinationPropagation {
    fn run_pass(&self, tcx: TyCtxt<'tcx>, source: MirSource<'tcx>, body: &mut Body<'tcx>) {
        // Only run at mir-opt-level=2 or higher for now (we don't fix up debuginfo and remove
        // storage statements at the moment).
        if tcx.sess.opts.debugging_opts.mir_opt_level <= 1 {
            return;
        }

        let candidates = find_candidates(tcx, body);
        if candidates.is_empty() {
            debug!("{:?}: no dest prop candidates, done", source.def_id());
            return;
        }

        // Collect all locals we care about. We only compute conflicts for these to save time.
        let mut relevant_locals = BitSet::new_empty(body.local_decls.len());
        for CandidateAssignment { dest, src, loc: _ } in &candidates {
            relevant_locals.insert(dest.local);
            relevant_locals.insert(*src);
        }

        // This pass unfortunately has `O(l² * s)` performance, where `l` is the number of locals
        // and `s` is the number of statements and terminators in the function.
        // To prevent blowing up compile times too much, we bail out when there are too many locals.
        let relevant = relevant_locals.count();
        debug!(
            "{:?}: {} locals ({} relevant), {} blocks",
            source.def_id(),
            body.local_decls.len(),
            relevant,
            body.basic_blocks().len()
        );
        if relevant > MAX_LOCALS {
            warn!(
                "too many candidate locals in {:?} ({}, max is {}), not optimizing",
                source.def_id(),
                relevant,
                MAX_LOCALS
            );
            return;
        }

        let mut conflicts = Conflicts::build(tcx, body, source, &relevant_locals);

        let mut replacements = Replacements::new(body.local_decls.len());
        for candidate @ CandidateAssignment { dest, src, loc } in candidates {
            // Merge locals that don't conflict.
            if conflicts.contains(dest.local, src) {
                debug!("at assignment {:?}, conflict {:?} vs. {:?}", loc, dest.local, src);
                continue;
            }

            if !tcx.consider_optimizing(|| {
                format!("DestinationPropagation {:?} {:?}", source.def_id(), candidate)
            }) {
                break;
            }

            if replacements.push(candidate).is_ok() {
                conflicts.unify(candidate.src, candidate.dest.local);
            }
        }

        replacements.flatten(tcx);

        debug!("replacements {:?}", replacements.map);

        Replacer { tcx, replacements, place_elem_cache: Vec::new() }.visit_body(body);

        // FIXME fix debug info
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
struct UnifyLocal(Local);

impl From<Local> for UnifyLocal {
    fn from(l: Local) -> Self {
        Self(l)
    }
}

impl UnifyKey for UnifyLocal {
    type Value = ();
    fn index(&self) -> u32 {
        self.0.as_u32()
    }
    fn from_index(u: u32) -> Self {
        Self(Local::from_u32(u))
    }
    fn tag() -> &'static str {
        "UnifyLocal"
    }
}

struct Replacements<'tcx> {
    /// Maps locals to their replacement.
    map: IndexVec<Local, Option<Place<'tcx>>>,

    /// Whose locals' live ranges to kill.
    kill: BitSet<Local>,

    /// Tracks locals that have already been merged together to prevent cycles.
    unified_locals: InPlaceUnificationTable<UnifyLocal>,
}

impl Replacements<'tcx> {
    fn new(locals: usize) -> Self {
        Self {
            map: IndexVec::from_elem_n(None, locals),
            kill: BitSet::new_empty(locals),
            unified_locals: {
                let mut table = InPlaceUnificationTable::new();
                for local in 0..locals {
                    assert_eq!(table.new_key(()), UnifyLocal(Local::from_usize(local)));
                }
                table
            },
        }
    }

    fn push(&mut self, candidate: CandidateAssignment<'tcx>) -> Result<(), ()> {
        if self.unified_locals.unioned(candidate.src, candidate.dest.local) {
            // Candidate conflicts with previous replacement (ie. could possibly form a cycle and
            // hang).

            let replacement = self.map[candidate.src].as_mut().unwrap();

            // If the current replacement is for the same `dest` local, there are 2 or more
            // equivalent `src = dest;` assignments. This is fine, the replacer will `nop` out all
            // of them.
            if replacement.local == candidate.dest.local {
                assert_eq!(replacement.projection, candidate.dest.projection);
            }

            // We still return `Err` in any case, as `src` and `dest` do not need to be unified
            // *again*.
            return Err(());
        }

        let entry = &mut self.map[candidate.src];
        if entry.is_some() {
            // We're already replacing `src` with something else, so this candidate is out.
            return Err(());
        }

        self.unified_locals.union(candidate.src, candidate.dest.local);

        *entry = Some(candidate.dest);
        self.kill.insert(candidate.src);
        self.kill.insert(candidate.dest.local);

        Ok(())
    }

    /// Applies the stored replacements to all replacements, until no replacements would result in
    /// locals that need further replacements when applied.
    fn flatten(&mut self, tcx: TyCtxt<'tcx>) {
        // Note: This assumes that there are no cycles in the replacements, which is enforced via
        // `self.unified_locals`. Otherwise this can cause an infinite loop.

        for local in self.map.indices() {
            if let Some(replacement) = self.map[local] {
                // Substitute the base local of `replacement` until fixpoint.
                let mut base = replacement.local;
                let mut reversed_projection_slices = Vec::with_capacity(1);
                while let Some(replacement_for_replacement) = self.map[base] {
                    base = replacement_for_replacement.local;
                    reversed_projection_slices.push(replacement_for_replacement.projection);
                }

                let projection: Vec<_> = reversed_projection_slices
                    .iter()
                    .rev()
                    .flat_map(|projs| projs.iter())
                    .chain(replacement.projection.iter())
                    .collect();
                let projection = tcx.intern_place_elems(&projection);

                // Replace with the final `Place`.
                self.map[local] = Some(Place { local: base, projection });
            }
        }
    }

    fn for_src(&self, src: Local) -> Option<&Place<'tcx>> {
        self.map[src].as_ref()
    }
}

struct Replacer<'tcx> {
    tcx: TyCtxt<'tcx>,
    replacements: Replacements<'tcx>,
    place_elem_cache: Vec<PlaceElem<'tcx>>,
}

impl<'tcx> MutVisitor<'tcx> for Replacer<'tcx> {
    fn tcx<'a>(&'a self) -> TyCtxt<'tcx> {
        self.tcx
    }

    fn visit_local(&mut self, local: &mut Local, context: PlaceContext, location: Location) {
        if context.is_use() && self.replacements.for_src(*local).is_some() {
            bug!(
                "use of local {:?} should have been replaced by visit_place; context={:?}, loc={:?}",
                local,
                context,
                location,
            );
        }
    }

    fn process_projection_elem(
        &mut self,
        elem: PlaceElem<'tcx>,
        _: Location,
    ) -> Option<PlaceElem<'tcx>> {
        match elem {
            PlaceElem::Index(local) => {
                if let Some(replacement) = self.replacements.for_src(local) {
                    bug!(
                        "cannot replace {:?} with {:?} in index projection {:?}",
                        local,
                        replacement,
                        elem,
                    );
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn visit_place(&mut self, place: &mut Place<'tcx>, context: PlaceContext, location: Location) {
        if let Some(replacement) = self.replacements.for_src(place.local) {
            // Rebase `place`s projections onto `replacement`'s.
            self.place_elem_cache.clear();
            self.place_elem_cache.extend(replacement.projection.iter().chain(place.projection));
            let projection = self.tcx.intern_place_elems(&self.place_elem_cache);
            let new_place = Place { local: replacement.local, projection };

            debug!("Replacer: {:?} -> {:?}", place, new_place);
            *place = new_place;
        }

        self.super_place(place, context, location);
    }

    fn visit_statement(&mut self, statement: &mut Statement<'tcx>, location: Location) {
        self.super_statement(statement, location);

        match &statement.kind {
            // FIXME: Don't delete storage statements, merge the live ranges instead
            StatementKind::StorageDead(local) | StatementKind::StorageLive(local)
                if self.replacements.kill.contains(*local) =>
            {
                statement.make_nop()
            }

            StatementKind::Assign(box (dest, rvalue)) => {
                match rvalue {
                    Rvalue::Use(Operand::Copy(place) | Operand::Move(place)) => {
                        // These might've been turned into self-assignments by the replacement
                        // (this includes the original statement we wanted to eliminate).
                        if dest == place {
                            debug!("{:?} turned into self-assignment, deleting", location);
                            statement.make_nop();
                        }
                    }
                    _ => {}
                }
            }

            _ => {}
        }
    }
}

struct Conflicts {
    /// The conflict matrix. It is always symmetric and the adjacency matrix of the corresponding
    /// conflict graph.
    matrix: BitMatrix<Local, Local>,

    /// Preallocated `BitSet` used by `unify`.
    unify_cache: BitSet<Local>,
}

impl Conflicts {
    fn build<'tcx>(
        tcx: TyCtxt<'tcx>,
        body: &'_ Body<'tcx>,
        source: MirSource<'tcx>,
        relevant_locals: &BitSet<Local>,
    ) -> Self {
        // We don't have to look out for locals that have their address taken, since
        // `find_candidates` already takes care of that.

        let mut conflicts = BitMatrix::from_row_n(
            &BitSet::new_empty(body.local_decls.len()),
            body.local_decls.len(),
        );

        let mut record_conflicts = |new_conflicts: &mut BitSet<_>| {
            // Remove all locals that are not candidates.
            new_conflicts.intersect(relevant_locals);

            for local in new_conflicts.iter() {
                conflicts.union_row_with(&new_conflicts, local);
            }
        };

        let def_id = source.def_id();
        let mut init = MaybeInitializedLocals
            .into_engine(tcx, body, def_id)
            .iterate_to_fixpoint()
            .into_results_cursor(body);
        let mut live = MaybeLiveLocals
            .into_engine(tcx, body, def_id)
            .iterate_to_fixpoint()
            .into_results_cursor(body);

        let mut reachable = None;
        dump_mir(
            tcx,
            None,
            "DestinationPropagation-dataflow",
            &"",
            source,
            body,
            |pass_where, w| {
                let reachable =
                    reachable.get_or_insert_with(|| traversal::reachable_as_bitset(body));

                match pass_where {
                    PassWhere::BeforeLocation(loc) if reachable.contains(loc.block) => {
                        init.seek_before_primary_effect(loc);
                        live.seek_after_primary_effect(loc);

                        writeln!(w, "        // init: {:?}", init.get())?;
                        writeln!(w, "        // live: {:?}", live.get())?;
                    }
                    PassWhere::AfterTerminator(bb) if reachable.contains(bb) => {
                        let loc = body.terminator_loc(bb);
                        init.seek_after_primary_effect(loc);
                        live.seek_before_primary_effect(loc);

                        writeln!(w, "        // init: {:?}", init.get())?;
                        writeln!(w, "        // live: {:?}", live.get())?;
                    }

                    PassWhere::BeforeBlock(bb) if reachable.contains(bb) => {
                        init.seek_to_block_start(bb);
                        live.seek_to_block_start(bb);

                        writeln!(w, "    // init: {:?}", init.get())?;
                        writeln!(w, "    // live: {:?}", live.get())?;
                    }

                    PassWhere::BeforeCFG | PassWhere::AfterCFG | PassWhere::AfterLocation(_) => {}

                    PassWhere::BeforeLocation(_) | PassWhere::AfterTerminator(_) => {
                        writeln!(w, "        // init: <unreachable>")?;
                        writeln!(w, "        // live: <unreachable>")?;
                    }

                    PassWhere::BeforeBlock(_) => {
                        writeln!(w, "    // init: <unreachable>")?;
                        writeln!(w, "    // live: <unreachable>")?;
                    }
                }

                Ok(())
            },
        );

        let mut live_and_init_locals = Vec::new();

        // Visit only reachable basic blocks. The exact order is not important.
        for (block, data) in traversal::preorder(body) {
            // We need to observe the dataflow state *before* all possible locations (statement or
            // terminator) in each basic block, and then observe the state *after* the terminator
            // effect is applied. As long as neither `init` nor `borrowed` has a "before" effect,
            // we will observe all possible dataflow states.

            // Since liveness is a backwards analysis, we need to walk the results backwards. To do
            // that, we first collect in the `MaybeInitializedLocals` results in a forwards
            // traversal.

            live_and_init_locals.resize_with(data.statements.len() + 1, || {
                BitSet::new_empty(body.local_decls.len())
            });

            // First, go forwards for `MaybeInitializedLocals`.
            for statement_index in 0..=data.statements.len() {
                let loc = Location { block, statement_index };
                init.seek_before_primary_effect(loc);

                live_and_init_locals[statement_index].clone_from(init.get());
            }

            // Now, go backwards and union with the liveness results.
            for statement_index in (0..=data.statements.len()).rev() {
                let loc = Location { block, statement_index };
                live.seek_after_primary_effect(loc);

                live_and_init_locals[statement_index].intersect(live.get());

                trace!("record conflicts at {:?}", loc);

                record_conflicts(&mut live_and_init_locals[statement_index]);
            }

            init.seek_to_block_end(block);
            live.seek_to_block_end(block);
            let mut conflicts = init.get().clone();
            conflicts.intersect(live.get());
            trace!("record conflicts at end of {:?}", block);

            record_conflicts(&mut conflicts);
        }

        Self { matrix: conflicts, unify_cache: BitSet::new_empty(body.local_decls.len()) }
    }

    fn contains(&self, a: Local, b: Local) -> bool {
        self.matrix.contains(a, b)
    }

    /// Merges the conflicts of `a` and `b`, so that each one inherits all conflicts of the other.
    ///
    /// This is called when the pass makes the decision to unify `a` and `b` (or parts of `a` and
    /// `b`) and is needed to ensure that future unification decisions take potentially newly
    /// introduced conflicts into account.
    ///
    /// For an example, assume we have locals `_0`, `_1`, `_2`, and `_3`. There are these conflicts:
    ///
    /// * `_0` <-> `_1`
    /// * `_1` <-> `_2`
    /// * `_3` <-> `_0`
    ///
    /// We then decide to merge `_2` with `_3` since they don't conflict. Then we decide to merge
    /// `_2` with `_0`, which also doesn't have a conflict in the above list. However `_2` is now
    /// `_3`, which does conflict with `_0`.
    fn unify(&mut self, a: Local, b: Local) {
        // FIXME: This might be somewhat slow. Conflict graphs are undirected, maybe we can use
        // something with union-find to speed this up?

        // Make all locals that conflict with `a` also conflict with `b`, and vice versa.
        self.unify_cache.clear();
        for conflicts_with_a in self.matrix.iter(a) {
            self.unify_cache.insert(conflicts_with_a);
        }
        for conflicts_with_b in self.matrix.iter(b) {
            self.unify_cache.insert(conflicts_with_b);
        }
        for conflicts_with_a_or_b in self.unify_cache.iter() {
            // Set both `a` and `b` for this local's row.
            self.matrix.insert(conflicts_with_a_or_b, a);
            self.matrix.insert(conflicts_with_a_or_b, b);
        }

        // Write the locals `a` conflicts with to `b`'s row.
        self.matrix.union_rows(a, b);
        // Write the locals `b` conflicts with to `a`'s row.
        self.matrix.union_rows(b, a);
    }
}

/// A `dest = {move} src;` statement at `loc`.
///
/// We want to consider merging `dest` and `src` due to this assignment.
#[derive(Debug, Copy, Clone)]
struct CandidateAssignment<'tcx> {
    /// Does not contain indirection or indexing (so the only local it contains is the place base).
    dest: Place<'tcx>,
    src: Local,
    loc: Location,
}

/// Scans the MIR for assignments between locals that we might want to consider merging.
///
/// This will filter out assignments that do not match the right form (as described in the top-level
/// comment) and also throw out assignments that involve a local that has its address taken or is
/// otherwise ineligible (eg. locals used as array indices are ignored because we cannot propagate
/// arbitrary places into array indices).
fn find_candidates<'a, 'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &'a Body<'tcx>,
) -> Vec<CandidateAssignment<'tcx>> {
    struct FindAssignments<'a, 'tcx> {
        tcx: TyCtxt<'tcx>,
        body: &'a Body<'tcx>,
        candidates: Vec<CandidateAssignment<'tcx>>,
        ever_borrowed_locals: BitSet<Local>,
        locals_used_as_array_index: BitSet<Local>,
    }

    impl<'a, 'tcx> Visitor<'tcx> for FindAssignments<'a, 'tcx> {
        fn visit_statement(&mut self, statement: &Statement<'tcx>, location: Location) {
            if let StatementKind::Assign(box (
                dest,
                Rvalue::Use(Operand::Copy(src) | Operand::Move(src)),
            )) = &statement.kind
            {
                // `dest` must not have pointer indirection.
                if dest.is_indirect() {
                    return;
                }

                // `src` must be a plain local.
                if !src.projection.is_empty() {
                    return;
                }

                // Since we want to replace `src` with `dest`, `src` must not be required.
                if is_local_required(src.local, self.body) {
                    return;
                }

                // Can't optimize if both locals ever have their address taken (can introduce
                // aliasing).
                // FIXME: This can be smarter and take `StorageDead` into account (which
                // invalidates borrows).
                if self.ever_borrowed_locals.contains(dest.local)
                    && self.ever_borrowed_locals.contains(src.local)
                {
                    return;
                }

                assert_ne!(dest.local, src.local, "self-assignments are UB");

                // We can't replace locals occurring in `PlaceElem::Index` for now.
                if self.locals_used_as_array_index.contains(src.local) {
                    return;
                }

                // Handle the "subtle case" described above by rejecting any `dest` that is or
                // projects through a union.
                let is_union = |ty: Ty<'_>| {
                    if let ty::Adt(def, _) = ty.kind() {
                        if def.is_union() {
                            return true;
                        }
                    }

                    false
                };
                let mut place_ty = PlaceTy::from_ty(self.body.local_decls[dest.local].ty);
                if is_union(place_ty.ty) {
                    return;
                }
                for elem in dest.projection {
                    if let PlaceElem::Index(_) = elem {
                        // `dest` contains an indexing projection.
                        return;
                    }

                    place_ty = place_ty.projection_ty(self.tcx, elem);
                    if is_union(place_ty.ty) {
                        return;
                    }
                }

                self.candidates.push(CandidateAssignment {
                    dest: *dest,
                    src: src.local,
                    loc: location,
                });
            }
        }
    }

    let mut visitor = FindAssignments {
        tcx,
        body,
        candidates: Vec::new(),
        ever_borrowed_locals: ever_borrowed_locals(body),
        locals_used_as_array_index: locals_used_as_array_index(body),
    };
    visitor.visit_body(body);
    visitor.candidates
}

/// Some locals are part of the function's interface and can not be removed.
///
/// Note that these locals *can* still be merged with non-required locals by removing that other
/// local.
fn is_local_required(local: Local, body: &Body<'_>) -> bool {
    match body.local_kind(local) {
        LocalKind::Arg | LocalKind::ReturnPointer => true,
        LocalKind::Var | LocalKind::Temp => false,
    }
}

/// Walks MIR to find all locals that have their address taken anywhere.
fn ever_borrowed_locals(body: &Body<'_>) -> BitSet<Local> {
    struct BorrowCollector {
        locals: BitSet<Local>,
    }

    impl<'tcx> Visitor<'tcx> for BorrowCollector {
        fn visit_rvalue(&mut self, rvalue: &Rvalue<'tcx>, location: Location) {
            self.super_rvalue(rvalue, location);

            match rvalue {
                Rvalue::AddressOf(_, borrowed_place) | Rvalue::Ref(_, _, borrowed_place) => {
                    if !borrowed_place.is_indirect() {
                        self.locals.insert(borrowed_place.local);
                    }
                }

                Rvalue::Cast(..)
                | Rvalue::Use(..)
                | Rvalue::Repeat(..)
                | Rvalue::Len(..)
                | Rvalue::BinaryOp(..)
                | Rvalue::CheckedBinaryOp(..)
                | Rvalue::NullaryOp(..)
                | Rvalue::UnaryOp(..)
                | Rvalue::Discriminant(..)
                | Rvalue::Aggregate(..)
                | Rvalue::ThreadLocalRef(..) => {}
            }
        }

        fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
            self.super_terminator(terminator, location);

            match terminator.kind {
                TerminatorKind::Drop { place: dropped_place, .. }
                | TerminatorKind::DropAndReplace { place: dropped_place, .. } => {
                    self.locals.insert(dropped_place.local);
                }

                TerminatorKind::Abort
                | TerminatorKind::Assert { .. }
                | TerminatorKind::Call { .. }
                | TerminatorKind::FalseEdge { .. }
                | TerminatorKind::FalseUnwind { .. }
                | TerminatorKind::GeneratorDrop
                | TerminatorKind::Goto { .. }
                | TerminatorKind::Resume
                | TerminatorKind::Return
                | TerminatorKind::SwitchInt { .. }
                | TerminatorKind::Unreachable
                | TerminatorKind::Yield { .. }
                | TerminatorKind::InlineAsm { .. } => {}
            }
        }
    }

    let mut visitor = BorrowCollector { locals: BitSet::new_empty(body.local_decls.len()) };
    visitor.visit_body(body);
    visitor.locals
}

/// `PlaceElem::Index` only stores a `Local`, so we can't replace that with a full `Place`.
///
/// Collect locals used as indices so we don't generate candidates that are impossible to apply
/// later.
fn locals_used_as_array_index(body: &Body<'_>) -> BitSet<Local> {
    struct IndexCollector {
        locals: BitSet<Local>,
    }

    impl<'tcx> Visitor<'tcx> for IndexCollector {
        fn visit_projection_elem(
            &mut self,
            local: Local,
            proj_base: &[PlaceElem<'tcx>],
            elem: PlaceElem<'tcx>,
            context: PlaceContext,
            location: Location,
        ) {
            if let PlaceElem::Index(i) = elem {
                self.locals.insert(i);
            }
            self.super_projection_elem(local, proj_base, elem, context, location);
        }
    }

    let mut visitor = IndexCollector { locals: BitSet::new_empty(body.local_decls.len()) };
    visitor.visit_body(body);
    visitor.locals
}
