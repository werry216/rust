// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! See README.md

use self::UndoLogEntry::*;
use self::CombineMapType::*;

use super::{MiscVariable, RegionVariableOrigin, SubregionOrigin};
use super::unify_key;

use rustc_data_structures::fx::{FxHashMap, FxHashSet};
use rustc_data_structures::unify::{self, UnificationTable};
use ty::{self, Ty, TyCtxt};
use ty::{Region, RegionVid};
use ty::ReStatic;
use ty::{BrFresh, ReLateBound, ReSkolemized, ReVar};

use std::collections::BTreeMap;
use std::fmt;
use std::mem;
use std::u32;

mod taint;

/// A constraint that influences the inference process.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Constraint<'tcx> {
    /// One region variable is subregion of another
    VarSubVar(RegionVid, RegionVid),

    /// Concrete region is subregion of region variable
    RegSubVar(Region<'tcx>, RegionVid),

    /// Region variable is subregion of concrete region. This does not
    /// directly affect inference, but instead is checked after
    /// inference is complete.
    VarSubReg(RegionVid, Region<'tcx>),

    /// A constraint where neither side is a variable. This does not
    /// directly affect inference, but instead is checked after
    /// inference is complete.
    RegSubReg(Region<'tcx>, Region<'tcx>),
}

/// VerifyGenericBound(T, _, R, RS): The parameter type `T` (or
/// associated type) must outlive the region `R`. `T` is known to
/// outlive `RS`. Therefore verify that `R <= RS[i]` for some
/// `i`. Inference variables may be involved (but this verification
/// step doesn't influence inference).
#[derive(Debug)]
pub struct Verify<'tcx> {
    pub kind: GenericKind<'tcx>,
    pub origin: SubregionOrigin<'tcx>,
    pub region: Region<'tcx>,
    pub bound: VerifyBound<'tcx>,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum GenericKind<'tcx> {
    Param(ty::ParamTy),
    Projection(ty::ProjectionTy<'tcx>),
}

/// When we introduce a verification step, we wish to test that a
/// particular region (let's call it `'min`) meets some bound.
/// The bound is described the by the following grammar:
#[derive(Debug)]
pub enum VerifyBound<'tcx> {
    /// B = exists {R} --> some 'r in {R} must outlive 'min
    ///
    /// Put another way, the subject value is known to outlive all
    /// regions in {R}, so if any of those outlives 'min, then the
    /// bound is met.
    AnyRegion(Vec<Region<'tcx>>),

    /// B = forall {R} --> all 'r in {R} must outlive 'min
    ///
    /// Put another way, the subject value is known to outlive some
    /// region in {R}, so if all of those outlives 'min, then the bound
    /// is met.
    AllRegions(Vec<Region<'tcx>>),

    /// B = exists {B} --> 'min must meet some bound b in {B}
    AnyBound(Vec<VerifyBound<'tcx>>),

    /// B = forall {B} --> 'min must meet all bounds b in {B}
    AllBounds(Vec<VerifyBound<'tcx>>),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
struct TwoRegions<'tcx> {
    a: Region<'tcx>,
    b: Region<'tcx>,
}

#[derive(Copy, Clone, PartialEq)]
enum UndoLogEntry<'tcx> {
    /// Pushed when we start a snapshot.
    OpenSnapshot,

    /// Replaces an `OpenSnapshot` when a snapshot is committed, but
    /// that snapshot is not the root. If the root snapshot is
    /// unrolled, all nested snapshots must be committed.
    CommitedSnapshot,

    /// We added `RegionVid`
    AddVar(RegionVid),

    /// We added the given `constraint`
    AddConstraint(Constraint<'tcx>),

    /// We added the given `verify`
    AddVerify(usize),

    /// We added the given `given`
    AddGiven(Region<'tcx>, ty::RegionVid),

    /// We added a GLB/LUB "combination variable"
    AddCombination(CombineMapType, TwoRegions<'tcx>),

    /// During skolemization, we sometimes purge entries from the undo
    /// log in a kind of minisnapshot (unlike other snapshots, this
    /// purging actually takes place *on success*). In that case, we
    /// replace the corresponding entry with `Noop` so as to avoid the
    /// need to do a bunch of swapping. (We can't use `swap_remove` as
    /// the order of the vector is important.)
    Purged,
}

#[derive(Copy, Clone, PartialEq)]
enum CombineMapType {
    Lub,
    Glb,
}

type CombineMap<'tcx> = FxHashMap<TwoRegions<'tcx>, RegionVid>;

pub struct RegionVarBindings<'tcx> {
    pub(in infer) var_origins: Vec<RegionVariableOrigin>,

    /// Constraints of the form `A <= B` introduced by the region
    /// checker.  Here at least one of `A` and `B` must be a region
    /// variable.
    ///
    /// Using `BTreeMap` because the order in which we iterate over
    /// these constraints can affect the way we build the region graph,
    /// which in turn affects the way that region errors are reported,
    /// leading to small variations in error output across runs and
    /// platforms.
    pub(in infer) constraints: BTreeMap<Constraint<'tcx>, SubregionOrigin<'tcx>>,

    /// A "verify" is something that we need to verify after inference is
    /// done, but which does not directly affect inference in any way.
    ///
    /// An example is a `A <= B` where neither `A` nor `B` are
    /// inference variables.
    pub(in infer) verifys: Vec<Verify<'tcx>>,

    /// A "given" is a relationship that is known to hold. In particular,
    /// we often know from closure fn signatures that a particular free
    /// region must be a subregion of a region variable:
    ///
    ///    foo.iter().filter(<'a> |x: &'a &'b T| ...)
    ///
    /// In situations like this, `'b` is in fact a region variable
    /// introduced by the call to `iter()`, and `'a` is a bound region
    /// on the closure (as indicated by the `<'a>` prefix). If we are
    /// naive, we wind up inferring that `'b` must be `'static`,
    /// because we require that it be greater than `'a` and we do not
    /// know what `'a` is precisely.
    ///
    /// This hashmap is used to avoid that naive scenario. Basically we
    /// record the fact that `'a <= 'b` is implied by the fn signature,
    /// and then ignore the constraint when solving equations. This is
    /// a bit of a hack but seems to work.
    pub(in infer) givens: FxHashSet<(Region<'tcx>, ty::RegionVid)>,

    lubs: CombineMap<'tcx>,
    glbs: CombineMap<'tcx>,
    skolemization_count: u32,
    bound_count: u32,

    /// The undo log records actions that might later be undone.
    ///
    /// Note: when the undo_log is empty, we are not actively
    /// snapshotting. When the `start_snapshot()` method is called, we
    /// push an OpenSnapshot entry onto the list to indicate that we
    /// are now actively snapshotting. The reason for this is that
    /// otherwise we end up adding entries for things like the lower
    /// bound on a variable and so forth, which can never be rolled
    /// back.
    undo_log: Vec<UndoLogEntry<'tcx>>,

    unification_table: UnificationTable<ty::RegionVid>,
}

pub struct RegionSnapshot {
    length: usize,
    region_snapshot: unify::Snapshot<ty::RegionVid>,
    skolemization_count: u32,
}

/// When working with skolemized regions, we often wish to find all of
/// the regions that are either reachable from a skolemized region, or
/// which can reach a skolemized region, or both. We call such regions
/// *tained* regions.  This struct allows you to decide what set of
/// tainted regions you want.
#[derive(Debug)]
pub struct TaintDirections {
    incoming: bool,
    outgoing: bool,
}

impl TaintDirections {
    pub fn incoming() -> Self {
        TaintDirections {
            incoming: true,
            outgoing: false,
        }
    }

    pub fn outgoing() -> Self {
        TaintDirections {
            incoming: false,
            outgoing: true,
        }
    }

    pub fn both() -> Self {
        TaintDirections {
            incoming: true,
            outgoing: true,
        }
    }
}

impl<'tcx> RegionVarBindings<'tcx> {
    pub fn new() -> RegionVarBindings<'tcx> {
        RegionVarBindings {
            var_origins: Vec::new(),
            constraints: BTreeMap::new(),
            verifys: Vec::new(),
            givens: FxHashSet(),
            lubs: FxHashMap(),
            glbs: FxHashMap(),
            skolemization_count: 0,
            bound_count: 0,
            undo_log: Vec::new(),
            unification_table: UnificationTable::new(),
        }
    }

    fn in_snapshot(&self) -> bool {
        !self.undo_log.is_empty()
    }

    pub fn start_snapshot(&mut self) -> RegionSnapshot {
        let length = self.undo_log.len();
        debug!("RegionVarBindings: start_snapshot({})", length);
        self.undo_log.push(OpenSnapshot);
        RegionSnapshot {
            length,
            region_snapshot: self.unification_table.snapshot(),
            skolemization_count: self.skolemization_count,
        }
    }

    pub fn commit(&mut self, snapshot: RegionSnapshot) {
        debug!("RegionVarBindings: commit({})", snapshot.length);
        assert!(self.undo_log.len() > snapshot.length);
        assert!(self.undo_log[snapshot.length] == OpenSnapshot);
        assert!(
            self.skolemization_count == snapshot.skolemization_count,
            "failed to pop skolemized regions: {} now vs {} at start",
            self.skolemization_count,
            snapshot.skolemization_count
        );

        if snapshot.length == 0 {
            self.undo_log.truncate(0);
        } else {
            (*self.undo_log)[snapshot.length] = CommitedSnapshot;
        }
        self.unification_table
            .commit(snapshot.region_snapshot);
    }

    pub fn rollback_to(&mut self, snapshot: RegionSnapshot) {
        debug!("RegionVarBindings: rollback_to({:?})", snapshot);
        assert!(self.undo_log.len() > snapshot.length);
        assert!(self.undo_log[snapshot.length] == OpenSnapshot);
        while self.undo_log.len() > snapshot.length + 1 {
            let undo_entry = self.undo_log.pop().unwrap();
            self.rollback_undo_entry(undo_entry);
        }
        let c = self.undo_log.pop().unwrap();
        assert!(c == OpenSnapshot);
        self.skolemization_count = snapshot.skolemization_count;
        self.unification_table
            .rollback_to(snapshot.region_snapshot);
    }

    fn rollback_undo_entry(&mut self, undo_entry: UndoLogEntry<'tcx>) {
        match undo_entry {
            OpenSnapshot => {
                panic!("Failure to observe stack discipline");
            }
            Purged | CommitedSnapshot => {
                // nothing to do here
            }
            AddVar(vid) => {
                self.var_origins.pop().unwrap();
                assert_eq!(self.var_origins.len(), vid.index as usize);
            }
            AddConstraint(ref constraint) => {
                self.constraints.remove(constraint);
            }
            AddVerify(index) => {
                self.verifys.pop();
                assert_eq!(self.verifys.len(), index);
            }
            AddGiven(sub, sup) => {
                self.givens.remove(&(sub, sup));
            }
            AddCombination(Glb, ref regions) => {
                self.glbs.remove(regions);
            }
            AddCombination(Lub, ref regions) => {
                self.lubs.remove(regions);
            }
        }
    }

    pub fn num_vars(&self) -> u32 {
        let len = self.var_origins.len();
        // enforce no overflow
        assert!(len as u32 as usize == len);
        len as u32
    }

    pub fn new_region_var(&mut self, origin: RegionVariableOrigin) -> RegionVid {
        let vid = RegionVid {
            index: self.num_vars(),
        };
        self.var_origins.push(origin.clone());

        let u_vid = self.unification_table
            .new_key(unify_key::RegionVidKey { min_vid: vid });
        assert_eq!(vid, u_vid);
        if self.in_snapshot() {
            self.undo_log.push(AddVar(vid));
        }
        debug!(
            "created new region variable {:?} with origin {:?}",
            vid,
            origin
        );
        return vid;
    }

    pub fn var_origin(&self, vid: RegionVid) -> RegionVariableOrigin {
        self.var_origins[vid.index as usize].clone()
    }

    /// Creates a new skolemized region. Skolemized regions are fresh
    /// regions used when performing higher-ranked computations. They
    /// must be used in a very particular way and are never supposed
    /// to "escape" out into error messages or the code at large.
    ///
    /// The idea is to always create a snapshot. Skolemized regions
    /// can be created in the context of this snapshot, but before the
    /// snapshot is committed or rolled back, they must be popped
    /// (using `pop_skolemized_regions`), so that their numbers can be
    /// recycled. Normally you don't have to think about this: you use
    /// the APIs in `higher_ranked/mod.rs`, such as
    /// `skolemize_late_bound_regions` and `plug_leaks`, which will
    /// guide you on this path (ensure that the `SkolemizationMap` is
    /// consumed and you are good).  There are also somewhat extensive
    /// comments in `higher_ranked/README.md`.
    ///
    /// The `snapshot` argument to this function is not really used;
    /// it's just there to make it explicit which snapshot bounds the
    /// skolemized region that results. It should always be the top-most snapshot.
    pub fn push_skolemized(
        &mut self,
        tcx: TyCtxt<'_, '_, 'tcx>,
        br: ty::BoundRegion,
        snapshot: &RegionSnapshot,
    ) -> Region<'tcx> {
        assert!(self.in_snapshot());
        assert!(self.undo_log[snapshot.length] == OpenSnapshot);

        let sc = self.skolemization_count;
        self.skolemization_count = sc + 1;
        tcx.mk_region(ReSkolemized(ty::SkolemizedRegionVid { index: sc }, br))
    }

    /// Removes all the edges to/from the skolemized regions that are
    /// in `skols`. This is used after a higher-ranked operation
    /// completes to remove all trace of the skolemized regions
    /// created in that time.
    pub fn pop_skolemized(
        &mut self,
        _tcx: TyCtxt<'_, '_, 'tcx>,
        skols: &FxHashSet<ty::Region<'tcx>>,
        snapshot: &RegionSnapshot,
    ) {
        debug!("pop_skolemized_regions(skols={:?})", skols);

        assert!(self.in_snapshot());
        assert!(self.undo_log[snapshot.length] == OpenSnapshot);
        assert!(
            self.skolemization_count as usize >= skols.len(),
            "popping more skolemized variables than actually exist, \
             sc now = {}, skols.len = {}",
            self.skolemization_count,
            skols.len()
        );

        let last_to_pop = self.skolemization_count;
        let first_to_pop = last_to_pop - (skols.len() as u32);

        assert!(
            first_to_pop >= snapshot.skolemization_count,
            "popping more regions than snapshot contains, \
             sc now = {}, sc then = {}, skols.len = {}",
            self.skolemization_count,
            snapshot.skolemization_count,
            skols.len()
        );
        debug_assert! {
            skols.iter()
                 .all(|&k| match *k {
                     ty::ReSkolemized(index, _) =>
                         index.index >= first_to_pop &&
                         index.index < last_to_pop,
                     _ =>
                         false
                 }),
            "invalid skolemization keys or keys out of range ({}..{}): {:?}",
            snapshot.skolemization_count,
            self.skolemization_count,
            skols
        }

        let constraints_to_kill: Vec<usize> = self.undo_log
            .iter()
            .enumerate()
            .rev()
            .filter(|&(_, undo_entry)| kill_constraint(skols, undo_entry))
            .map(|(index, _)| index)
            .collect();

        for index in constraints_to_kill {
            let undo_entry = mem::replace(&mut self.undo_log[index], Purged);
            self.rollback_undo_entry(undo_entry);
        }

        self.skolemization_count = snapshot.skolemization_count;
        return;

        fn kill_constraint<'tcx>(
            skols: &FxHashSet<ty::Region<'tcx>>,
            undo_entry: &UndoLogEntry<'tcx>,
        ) -> bool {
            match undo_entry {
                &AddConstraint(Constraint::VarSubVar(..)) => false,
                &AddConstraint(Constraint::RegSubVar(a, _)) => skols.contains(&a),
                &AddConstraint(Constraint::VarSubReg(_, b)) => skols.contains(&b),
                &AddConstraint(Constraint::RegSubReg(a, b)) => {
                    skols.contains(&a) || skols.contains(&b)
                }
                &AddGiven(..) => false,
                &AddVerify(_) => false,
                &AddCombination(_, ref two_regions) => {
                    skols.contains(&two_regions.a) || skols.contains(&two_regions.b)
                }
                &AddVar(..) | &OpenSnapshot | &Purged | &CommitedSnapshot => false,
            }
        }
    }

    pub fn new_bound(
        &mut self,
        tcx: TyCtxt<'_, '_, 'tcx>,
        debruijn: ty::DebruijnIndex,
    ) -> Region<'tcx> {
        // Creates a fresh bound variable for use in GLB computations.
        // See discussion of GLB computation in the large comment at
        // the top of this file for more details.
        //
        // This computation is potentially wrong in the face of
        // rollover.  It's conceivable, if unlikely, that one might
        // wind up with accidental capture for nested functions in
        // that case, if the outer function had bound regions created
        // a very long time before and the inner function somehow
        // wound up rolling over such that supposedly fresh
        // identifiers were in fact shadowed. For now, we just assert
        // that there is no rollover -- eventually we should try to be
        // robust against this possibility, either by checking the set
        // of bound identifiers that appear in a given expression and
        // ensure that we generate one that is distinct, or by
        // changing the representation of bound regions in a fn
        // declaration

        let sc = self.bound_count;
        self.bound_count = sc + 1;

        if sc >= self.bound_count {
            bug!("rollover in RegionInference new_bound()");
        }

        tcx.mk_region(ReLateBound(debruijn, BrFresh(sc)))
    }

    fn add_constraint(&mut self, constraint: Constraint<'tcx>, origin: SubregionOrigin<'tcx>) {
        // cannot add constraints once regions are resolved
        debug!("RegionVarBindings: add_constraint({:?})", constraint);

        // never overwrite an existing (constraint, origin) - only insert one if it isn't
        // present in the map yet. This prevents origins from outside the snapshot being
        // replaced with "less informative" origins e.g. during calls to `can_eq`
        let in_snapshot = self.in_snapshot();
        let undo_log = &mut self.undo_log;
        self.constraints
            .entry(constraint)
            .or_insert_with(|| {
                if in_snapshot {
                    undo_log.push(AddConstraint(constraint));
                }
                origin
            });
    }

    fn add_verify(&mut self, verify: Verify<'tcx>) {
        // cannot add verifys once regions are resolved
        debug!("RegionVarBindings: add_verify({:?})", verify);

        // skip no-op cases known to be satisfied
        match verify.bound {
            VerifyBound::AllBounds(ref bs) if bs.len() == 0 => {
                return;
            }
            _ => {}
        }

        let index = self.verifys.len();
        self.verifys.push(verify);
        if self.in_snapshot() {
            self.undo_log.push(AddVerify(index));
        }
    }

    pub fn add_given(&mut self, sub: Region<'tcx>, sup: ty::RegionVid) {
        // cannot add givens once regions are resolved
        if self.givens.insert((sub, sup)) {
            debug!("add_given({:?} <= {:?})", sub, sup);

            self.undo_log.push(AddGiven(sub, sup));
        }
    }

    pub fn make_eqregion(
        &mut self,
        origin: SubregionOrigin<'tcx>,
        sub: Region<'tcx>,
        sup: Region<'tcx>,
    ) {
        if sub != sup {
            // Eventually, it would be nice to add direct support for
            // equating regions.
            self.make_subregion(origin.clone(), sub, sup);
            self.make_subregion(origin, sup, sub);

            if let (ty::ReVar(sub), ty::ReVar(sup)) = (*sub, *sup) {
                self.unification_table.union(sub, sup);
            }
        }
    }

    pub fn make_subregion(
        &mut self,
        origin: SubregionOrigin<'tcx>,
        sub: Region<'tcx>,
        sup: Region<'tcx>,
    ) {
        // cannot add constraints once regions are resolved
        debug!(
            "RegionVarBindings: make_subregion({:?}, {:?}) due to {:?}",
            sub,
            sup,
            origin
        );

        match (sub, sup) {
            (&ReLateBound(..), _) | (_, &ReLateBound(..)) => {
                span_bug!(
                    origin.span(),
                    "cannot relate bound region: {:?} <= {:?}",
                    sub,
                    sup
                );
            }
            (_, &ReStatic) => {
                // all regions are subregions of static, so we can ignore this
            }
            (&ReVar(sub_id), &ReVar(sup_id)) => {
                self.add_constraint(Constraint::VarSubVar(sub_id, sup_id), origin);
            }
            (_, &ReVar(sup_id)) => {
                self.add_constraint(Constraint::RegSubVar(sub, sup_id), origin);
            }
            (&ReVar(sub_id), _) => {
                self.add_constraint(Constraint::VarSubReg(sub_id, sup), origin);
            }
            _ => {
                self.add_constraint(Constraint::RegSubReg(sub, sup), origin);
            }
        }
    }

    /// See `Verify::VerifyGenericBound`
    pub fn verify_generic_bound(
        &mut self,
        origin: SubregionOrigin<'tcx>,
        kind: GenericKind<'tcx>,
        sub: Region<'tcx>,
        bound: VerifyBound<'tcx>,
    ) {
        self.add_verify(Verify {
            kind,
            origin,
            region: sub,
            bound,
        });
    }

    pub fn lub_regions(
        &mut self,
        tcx: TyCtxt<'_, '_, 'tcx>,
        origin: SubregionOrigin<'tcx>,
        a: Region<'tcx>,
        b: Region<'tcx>,
    ) -> Region<'tcx> {
        // cannot add constraints once regions are resolved
        debug!("RegionVarBindings: lub_regions({:?}, {:?})", a, b);
        match (a, b) {
            (r @ &ReStatic, _) | (_, r @ &ReStatic) => {
                r // nothing lives longer than static
            }

            _ if a == b => {
                a // LUB(a,a) = a
            }

            _ => self.combine_vars(tcx, Lub, a, b, origin.clone()),
        }
    }

    pub fn glb_regions(
        &mut self,
        tcx: TyCtxt<'_, '_, 'tcx>,
        origin: SubregionOrigin<'tcx>,
        a: Region<'tcx>,
        b: Region<'tcx>,
    ) -> Region<'tcx> {
        // cannot add constraints once regions are resolved
        debug!("RegionVarBindings: glb_regions({:?}, {:?})", a, b);
        match (a, b) {
            (&ReStatic, r) | (r, &ReStatic) => {
                r // static lives longer than everything else
            }

            _ if a == b => {
                a // GLB(a,a) = a
            }

            _ => self.combine_vars(tcx, Glb, a, b, origin.clone()),
        }
    }

    pub fn opportunistic_resolve_var(
        &mut self,
        tcx: TyCtxt<'_, '_, 'tcx>,
        rid: RegionVid,
    ) -> ty::Region<'tcx> {
        let vid = self.unification_table.find_value(rid).min_vid;
        tcx.mk_region(ty::ReVar(vid))
    }

    fn combine_map(&mut self, t: CombineMapType) -> &mut CombineMap<'tcx> {
        match t {
            Glb => &mut self.glbs,
            Lub => &mut self.lubs,
        }
    }

    fn combine_vars(
        &mut self,
        tcx: TyCtxt<'_, '_, 'tcx>,
        t: CombineMapType,
        a: Region<'tcx>,
        b: Region<'tcx>,
        origin: SubregionOrigin<'tcx>,
    ) -> Region<'tcx> {
        let vars = TwoRegions { a: a, b: b };
        if let Some(&c) = self.combine_map(t).get(&vars) {
            return tcx.mk_region(ReVar(c));
        }
        let c = self.new_region_var(MiscVariable(origin.span()));
        self.combine_map(t).insert(vars, c);
        if self.in_snapshot() {
            self.undo_log.push(AddCombination(t, vars));
        }
        let new_r = tcx.mk_region(ReVar(c));
        for &old_r in &[a, b] {
            match t {
                Glb => self.make_subregion(origin.clone(), new_r, old_r),
                Lub => self.make_subregion(origin.clone(), old_r, new_r),
            }
        }
        debug!("combine_vars() c={:?}", c);
        new_r
    }

    pub fn vars_created_since_snapshot(&self, mark: &RegionSnapshot) -> Vec<RegionVid> {
        self.undo_log[mark.length..]
            .iter()
            .filter_map(|&elt| match elt {
                AddVar(vid) => Some(vid),
                _ => None,
            })
            .collect()
    }

    /// Computes all regions that have been related to `r0` since the
    /// mark `mark` was made---`r0` itself will be the first
    /// entry. The `directions` parameter controls what kind of
    /// relations are considered. For example, one can say that only
    /// "incoming" edges to `r0` are desired, in which case one will
    /// get the set of regions `{r|r <= r0}`. This is used when
    /// checking whether skolemized regions are being improperly
    /// related to other regions.
    pub fn tainted(
        &self,
        tcx: TyCtxt<'_, '_, 'tcx>,
        mark: &RegionSnapshot,
        r0: Region<'tcx>,
        directions: TaintDirections,
    ) -> FxHashSet<ty::Region<'tcx>> {
        debug!(
            "tainted(mark={:?}, r0={:?}, directions={:?})",
            mark,
            r0,
            directions
        );

        // `result_set` acts as a worklist: we explore all outgoing
        // edges and add any new regions we find to result_set.  This
        // is not a terribly efficient implementation.
        let mut taint_set = taint::TaintSet::new(directions, r0);
        taint_set.fixed_point(
            tcx,
            &self.undo_log[mark.length..],
            &self.verifys,
        );
        debug!("tainted: result={:?}", taint_set);
        return taint_set.into_set();
    }
}

impl fmt::Debug for RegionSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "RegionSnapshot(length={},skolemization={})",
            self.length,
            self.skolemization_count
        )
    }
}

impl<'tcx> fmt::Debug for GenericKind<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            GenericKind::Param(ref p) => write!(f, "{:?}", p),
            GenericKind::Projection(ref p) => write!(f, "{:?}", p),
        }
    }
}

impl<'tcx> fmt::Display for GenericKind<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            GenericKind::Param(ref p) => write!(f, "{}", p),
            GenericKind::Projection(ref p) => write!(f, "{}", p),
        }
    }
}

impl<'a, 'gcx, 'tcx> GenericKind<'tcx> {
    pub fn to_ty(&self, tcx: TyCtxt<'a, 'gcx, 'tcx>) -> Ty<'tcx> {
        match *self {
            GenericKind::Param(ref p) => p.to_ty(tcx),
            GenericKind::Projection(ref p) => tcx.mk_projection(p.item_def_id, p.substs),
        }
    }
}

impl<'a, 'gcx, 'tcx> VerifyBound<'tcx> {
    fn for_each_region(&self, f: &mut FnMut(ty::Region<'tcx>)) {
        match self {
            &VerifyBound::AnyRegion(ref rs) | &VerifyBound::AllRegions(ref rs) => for &r in rs {
                f(r);
            },

            &VerifyBound::AnyBound(ref bs) | &VerifyBound::AllBounds(ref bs) => for b in bs {
                b.for_each_region(f);
            },
        }
    }

    pub fn must_hold(&self) -> bool {
        match self {
            &VerifyBound::AnyRegion(ref bs) => bs.contains(&&ty::ReStatic),
            &VerifyBound::AllRegions(ref bs) => bs.is_empty(),
            &VerifyBound::AnyBound(ref bs) => bs.iter().any(|b| b.must_hold()),
            &VerifyBound::AllBounds(ref bs) => bs.iter().all(|b| b.must_hold()),
        }
    }

    pub fn cannot_hold(&self) -> bool {
        match self {
            &VerifyBound::AnyRegion(ref bs) => bs.is_empty(),
            &VerifyBound::AllRegions(ref bs) => bs.contains(&&ty::ReEmpty),
            &VerifyBound::AnyBound(ref bs) => bs.iter().all(|b| b.cannot_hold()),
            &VerifyBound::AllBounds(ref bs) => bs.iter().any(|b| b.cannot_hold()),
        }
    }

    pub fn or(self, vb: VerifyBound<'tcx>) -> VerifyBound<'tcx> {
        if self.must_hold() || vb.cannot_hold() {
            self
        } else if self.cannot_hold() || vb.must_hold() {
            vb
        } else {
            VerifyBound::AnyBound(vec![self, vb])
        }
    }

    pub fn and(self, vb: VerifyBound<'tcx>) -> VerifyBound<'tcx> {
        if self.must_hold() && vb.must_hold() {
            self
        } else if self.cannot_hold() && vb.cannot_hold() {
            self
        } else {
            VerifyBound::AllBounds(vec![self, vb])
        }
    }
}
