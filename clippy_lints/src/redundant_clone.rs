use crate::utils::{
    fn_has_unsatisfiable_preds, has_drop, is_copy, is_type_diagnostic_item, match_def_path, match_type, paths,
    snippet_opt, span_lint_hir, span_lint_hir_and_then, walk_ptrs_ty_depth,
};
use if_chain::if_chain;
use rustc_data_structures::{fx::FxHashMap, transitive_relation::TransitiveRelation};
use rustc_errors::Applicability;
use rustc_hir::intravisit::FnKind;
use rustc_hir::{def_id, Body, FnDecl, HirId};
use rustc_index::bit_set::{BitSet, HybridBitSet};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::mir::{
    self, traversal,
    visit::{MutatingUseContext, NonMutatingUseContext, PlaceContext, Visitor as _},
};
use rustc_middle::ty::{self, fold::TypeVisitor, Ty};
use rustc_mir::dataflow::{Analysis, AnalysisDomain, GenKill, GenKillAnalysis, ResultsCursor};
use rustc_session::{declare_lint_pass, declare_tool_lint};
use rustc_span::source_map::{BytePos, Span};
use std::convert::TryFrom;

macro_rules! unwrap_or_continue {
    ($x:expr) => {
        match $x {
            Some(x) => x,
            None => continue,
        }
    };
}

declare_clippy_lint! {
    /// **What it does:** Checks for a redundant `clone()` (and its relatives) which clones an owned
    /// value that is going to be dropped without further use.
    ///
    /// **Why is this bad?** It is not always possible for the compiler to eliminate useless
    /// allocations and deallocations generated by redundant `clone()`s.
    ///
    /// **Known problems:**
    ///
    /// False-negatives: analysis performed by this lint is conservative and limited.
    ///
    /// **Example:**
    /// ```rust
    /// # use std::path::Path;
    /// # #[derive(Clone)]
    /// # struct Foo;
    /// # impl Foo {
    /// #     fn new() -> Self { Foo {} }
    /// # }
    /// # fn call(x: Foo) {}
    /// {
    ///     let x = Foo::new();
    ///     call(x.clone());
    ///     call(x.clone()); // this can just pass `x`
    /// }
    ///
    /// ["lorem", "ipsum"].join(" ").to_string();
    ///
    /// Path::new("/a/b").join("c").to_path_buf();
    /// ```
    pub REDUNDANT_CLONE,
    perf,
    "`clone()` of an owned value that is going to be dropped immediately"
}

declare_lint_pass!(RedundantClone => [REDUNDANT_CLONE]);

impl<'tcx> LateLintPass<'tcx> for RedundantClone {
    #[allow(clippy::too_many_lines)]
    fn check_fn(
        &mut self,
        cx: &LateContext<'tcx>,
        _: FnKind<'tcx>,
        _: &'tcx FnDecl<'_>,
        body: &'tcx Body<'_>,
        _: Span,
        _: HirId,
    ) {
        let def_id = cx.tcx.hir().body_owner_def_id(body.id());

        // Building MIR for `fn`s with unsatisfiable preds results in ICE.
        if fn_has_unsatisfiable_preds(cx, def_id.to_def_id()) {
            return;
        }

        let mir = cx.tcx.optimized_mir(def_id.to_def_id());

        let maybe_storage_live_result = MaybeStorageLive
            .into_engine(cx.tcx, mir, def_id.to_def_id())
            .pass_name("redundant_clone")
            .iterate_to_fixpoint()
            .into_results_cursor(mir);
        let mut possible_borrower = {
            let mut vis = PossibleBorrowerVisitor::new(cx, mir);
            vis.visit_body(&mir);
            vis.into_map(cx, maybe_storage_live_result)
        };

        for (bb, bbdata) in mir.basic_blocks().iter_enumerated() {
            let terminator = bbdata.terminator();

            if terminator.source_info.span.from_expansion() {
                continue;
            }

            // Give up on loops
            if terminator.successors().any(|s| *s == bb) {
                continue;
            }

            let (fn_def_id, arg, arg_ty, clone_ret) =
                unwrap_or_continue!(is_call_with_ref_arg(cx, mir, &terminator.kind));

            let from_borrow = match_def_path(cx, fn_def_id, &paths::CLONE_TRAIT_METHOD)
                || match_def_path(cx, fn_def_id, &paths::TO_OWNED_METHOD)
                || (match_def_path(cx, fn_def_id, &paths::TO_STRING_METHOD)
                    && is_type_diagnostic_item(cx, arg_ty, sym!(string_type)));

            let from_deref = !from_borrow
                && (match_def_path(cx, fn_def_id, &paths::PATH_TO_PATH_BUF)
                    || match_def_path(cx, fn_def_id, &paths::OS_STR_TO_OS_STRING));

            if !from_borrow && !from_deref {
                continue;
            }

            if let ty::Adt(ref def, _) = arg_ty.kind() {
                if match_def_path(cx, def.did, &paths::MEM_MANUALLY_DROP) {
                    continue;
                }
            }

            // `{ cloned = &arg; clone(move cloned); }` or `{ cloned = &arg; to_path_buf(cloned); }`
            let (cloned, cannot_move_out) = unwrap_or_continue!(find_stmt_assigns_to(cx, mir, arg, from_borrow, bb));

            let loc = mir::Location {
                block: bb,
                statement_index: bbdata.statements.len(),
            };

            // `Local` to be cloned, and a local of `clone` call's destination
            let (local, ret_local) = if from_borrow {
                // `res = clone(arg)` can be turned into `res = move arg;`
                // if `arg` is the only borrow of `cloned` at this point.

                if cannot_move_out || !possible_borrower.only_borrowers(&[arg], cloned, loc) {
                    continue;
                }

                (cloned, clone_ret)
            } else {
                // `arg` is a reference as it is `.deref()`ed in the previous block.
                // Look into the predecessor block and find out the source of deref.

                let ps = &mir.predecessors()[bb];
                if ps.len() != 1 {
                    continue;
                }
                let pred_terminator = mir[ps[0]].terminator();

                // receiver of the `deref()` call
                let (pred_arg, deref_clone_ret) = if_chain! {
                    if let Some((pred_fn_def_id, pred_arg, pred_arg_ty, res)) =
                        is_call_with_ref_arg(cx, mir, &pred_terminator.kind);
                    if res == cloned;
                    if match_def_path(cx, pred_fn_def_id, &paths::DEREF_TRAIT_METHOD);
                    if match_type(cx, pred_arg_ty, &paths::PATH_BUF)
                        || match_type(cx, pred_arg_ty, &paths::OS_STRING);
                    then {
                        (pred_arg, res)
                    } else {
                        continue;
                    }
                };

                let (local, cannot_move_out) =
                    unwrap_or_continue!(find_stmt_assigns_to(cx, mir, pred_arg, true, ps[0]));
                let loc = mir::Location {
                    block: bb,
                    statement_index: mir.basic_blocks()[bb].statements.len(),
                };

                // This can be turned into `res = move local` if `arg` and `cloned` are not borrowed
                // at the last statement:
                //
                // ```
                // pred_arg = &local;
                // cloned = deref(pred_arg);
                // arg = &cloned;
                // StorageDead(pred_arg);
                // res = to_path_buf(cloned);
                // ```
                if cannot_move_out || !possible_borrower.only_borrowers(&[arg, cloned], local, loc) {
                    continue;
                }

                (local, deref_clone_ret)
            };

            let is_temp = mir.local_kind(ret_local) == mir::LocalKind::Temp;

            // 1. `local` can be moved out if it is not used later.
            // 2. If `ret_local` is a temporary and is neither consumed nor mutated, we can remove this `clone`
            // call anyway.
            let (used, consumed_or_mutated) = traversal::ReversePostorder::new(&mir, bb).skip(1).fold(
                (false, !is_temp),
                |(used, consumed), (tbb, tdata)| {
                    // Short-circuit
                    if (used && consumed) ||
                        // Give up on loops
                        tdata.terminator().successors().any(|s| *s == bb)
                    {
                        return (true, true);
                    }

                    let mut vis = LocalUseVisitor {
                        used: (local, false),
                        consumed_or_mutated: (ret_local, false),
                    };
                    vis.visit_basic_block_data(tbb, tdata);
                    (used || vis.used.1, consumed || vis.consumed_or_mutated.1)
                },
            );

            if !used || !consumed_or_mutated {
                let span = terminator.source_info.span;
                let scope = terminator.source_info.scope;
                let node = mir.source_scopes[scope]
                    .local_data
                    .as_ref()
                    .assert_crate_local()
                    .lint_root;

                if_chain! {
                    if let Some(snip) = snippet_opt(cx, span);
                    if let Some(dot) = snip.rfind('.');
                    then {
                        let sugg_span = span.with_lo(
                            span.lo() + BytePos(u32::try_from(dot).unwrap())
                        );
                        let mut app = Applicability::MaybeIncorrect;

                        let mut call_snip = &snip[dot + 1..];
                        // Machine applicable when `call_snip` looks like `foobar()`
                        if call_snip.ends_with("()") {
                            call_snip = call_snip[..call_snip.len()-2].trim();
                            if call_snip.as_bytes().iter().all(|b| b.is_ascii_alphabetic() || *b == b'_') {
                                app = Applicability::MachineApplicable;
                            }
                        }

                        span_lint_hir_and_then(cx, REDUNDANT_CLONE, node, sugg_span, "redundant clone", |diag| {
                            diag.span_suggestion(
                                sugg_span,
                                "remove this",
                                String::new(),
                                app,
                            );
                            if used {
                                diag.span_note(
                                    span,
                                    "cloned value is neither consumed nor mutated",
                                );
                            } else {
                                diag.span_note(
                                    span.with_hi(span.lo() + BytePos(u32::try_from(dot).unwrap())),
                                    "this value is dropped without further use",
                                );
                            }
                        });
                    } else {
                        span_lint_hir(cx, REDUNDANT_CLONE, node, span, "redundant clone");
                    }
                }
            }
        }
    }
}

/// If `kind` is `y = func(x: &T)` where `T: !Copy`, returns `(DefId of func, x, T, y)`.
fn is_call_with_ref_arg<'tcx>(
    cx: &LateContext<'tcx>,
    mir: &'tcx mir::Body<'tcx>,
    kind: &'tcx mir::TerminatorKind<'tcx>,
) -> Option<(def_id::DefId, mir::Local, Ty<'tcx>, mir::Local)> {
    if_chain! {
        if let mir::TerminatorKind::Call { func, args, destination, .. } = kind;
        if args.len() == 1;
        if let mir::Operand::Move(mir::Place { local, .. }) = &args[0];
        if let ty::FnDef(def_id, _) = *func.ty(&*mir, cx.tcx).kind();
        if let (inner_ty, 1) = walk_ptrs_ty_depth(args[0].ty(&*mir, cx.tcx));
        if !is_copy(cx, inner_ty);
        then {
            Some((def_id, *local, inner_ty, destination.as_ref().map(|(dest, _)| dest)?.as_local()?))
        } else {
            None
        }
    }
}

type CannotMoveOut = bool;

/// Finds the first `to = (&)from`, and returns
/// ``Some((from, whether `from` cannot be moved out))``.
fn find_stmt_assigns_to<'tcx>(
    cx: &LateContext<'tcx>,
    mir: &mir::Body<'tcx>,
    to_local: mir::Local,
    by_ref: bool,
    bb: mir::BasicBlock,
) -> Option<(mir::Local, CannotMoveOut)> {
    let rvalue = mir.basic_blocks()[bb].statements.iter().rev().find_map(|stmt| {
        if let mir::StatementKind::Assign(box (mir::Place { local, .. }, v)) = &stmt.kind {
            return if *local == to_local { Some(v) } else { None };
        }

        None
    })?;

    match (by_ref, &*rvalue) {
        (true, mir::Rvalue::Ref(_, _, place)) | (false, mir::Rvalue::Use(mir::Operand::Copy(place))) => {
            base_local_and_movability(cx, mir, *place)
        },
        (false, mir::Rvalue::Ref(_, _, place)) => {
            if let [mir::ProjectionElem::Deref] = place.as_ref().projection {
                base_local_and_movability(cx, mir, *place)
            } else {
                None
            }
        },
        _ => None,
    }
}

/// Extracts and returns the undermost base `Local` of given `place`. Returns `place` itself
/// if it is already a `Local`.
///
/// Also reports whether given `place` cannot be moved out.
fn base_local_and_movability<'tcx>(
    cx: &LateContext<'tcx>,
    mir: &mir::Body<'tcx>,
    place: mir::Place<'tcx>,
) -> Option<(mir::Local, CannotMoveOut)> {
    use rustc_middle::mir::PlaceRef;

    // Dereference. You cannot move things out from a borrowed value.
    let mut deref = false;
    // Accessing a field of an ADT that has `Drop`. Moving the field out will cause E0509.
    let mut field = false;
    // If projection is a slice index then clone can be removed only if the
    // underlying type implements Copy
    let mut slice = false;

    let PlaceRef { local, mut projection } = place.as_ref();
    while let [base @ .., elem] = projection {
        projection = base;
        deref |= matches!(elem, mir::ProjectionElem::Deref);
        field |= matches!(elem, mir::ProjectionElem::Field(..))
            && has_drop(cx, mir::Place::ty_from(local, projection, &mir.local_decls, cx.tcx).ty);
        slice |= matches!(elem, mir::ProjectionElem::Index(..))
            && !is_copy(cx, mir::Place::ty_from(local, projection, &mir.local_decls, cx.tcx).ty);
    }

    Some((local, deref || field || slice))
}

struct LocalUseVisitor {
    used: (mir::Local, bool),
    consumed_or_mutated: (mir::Local, bool),
}

impl<'tcx> mir::visit::Visitor<'tcx> for LocalUseVisitor {
    fn visit_basic_block_data(&mut self, block: mir::BasicBlock, data: &mir::BasicBlockData<'tcx>) {
        let statements = &data.statements;
        for (statement_index, statement) in statements.iter().enumerate() {
            self.visit_statement(statement, mir::Location { block, statement_index });
        }

        self.visit_terminator(
            data.terminator(),
            mir::Location {
                block,
                statement_index: statements.len(),
            },
        );
    }

    fn visit_place(&mut self, place: &mir::Place<'tcx>, ctx: PlaceContext, _: mir::Location) {
        let local = place.local;

        if local == self.used.0
            && !matches!(ctx, PlaceContext::MutatingUse(MutatingUseContext::Drop) | PlaceContext::NonUse(_))
        {
            self.used.1 = true;
        }

        if local == self.consumed_or_mutated.0 {
            match ctx {
                PlaceContext::NonMutatingUse(NonMutatingUseContext::Move)
                | PlaceContext::MutatingUse(MutatingUseContext::Borrow) => {
                    self.consumed_or_mutated.1 = true;
                },
                _ => {},
            }
        }
    }
}

/// Determines liveness of each local purely based on `StorageLive`/`Dead`.
#[derive(Copy, Clone)]
struct MaybeStorageLive;

impl<'tcx> AnalysisDomain<'tcx> for MaybeStorageLive {
    type Domain = BitSet<mir::Local>;
    const NAME: &'static str = "maybe_storage_live";

    fn bottom_value(&self, body: &mir::Body<'tcx>) -> Self::Domain {
        // bottom = dead
        BitSet::new_empty(body.local_decls.len())
    }

    fn initialize_start_block(&self, body: &mir::Body<'tcx>, state: &mut Self::Domain) {
        for arg in body.args_iter() {
            state.insert(arg);
        }
    }
}

impl<'tcx> GenKillAnalysis<'tcx> for MaybeStorageLive {
    type Idx = mir::Local;

    fn statement_effect(&self, trans: &mut impl GenKill<Self::Idx>, stmt: &mir::Statement<'tcx>, _: mir::Location) {
        match stmt.kind {
            mir::StatementKind::StorageLive(l) => trans.gen(l),
            mir::StatementKind::StorageDead(l) => trans.kill(l),
            _ => (),
        }
    }

    fn terminator_effect(
        &self,
        _trans: &mut impl GenKill<Self::Idx>,
        _terminator: &mir::Terminator<'tcx>,
        _loc: mir::Location,
    ) {
    }

    fn call_return_effect(
        &self,
        _in_out: &mut impl GenKill<Self::Idx>,
        _block: mir::BasicBlock,
        _func: &mir::Operand<'tcx>,
        _args: &[mir::Operand<'tcx>],
        _return_place: mir::Place<'tcx>,
    ) {
        // Nothing to do when a call returns successfully
    }
}

/// Collects the possible borrowers of each local.
/// For example, `b = &a; c = &a;` will make `b` and (transitively) `c`
/// possible borrowers of `a`.
struct PossibleBorrowerVisitor<'a, 'tcx> {
    possible_borrower: TransitiveRelation<mir::Local>,
    body: &'a mir::Body<'tcx>,
    cx: &'a LateContext<'tcx>,
}

impl<'a, 'tcx> PossibleBorrowerVisitor<'a, 'tcx> {
    fn new(cx: &'a LateContext<'tcx>, body: &'a mir::Body<'tcx>) -> Self {
        Self {
            possible_borrower: TransitiveRelation::default(),
            cx,
            body,
        }
    }

    fn into_map(
        self,
        cx: &LateContext<'tcx>,
        maybe_live: ResultsCursor<'tcx, 'tcx, MaybeStorageLive>,
    ) -> PossibleBorrowerMap<'a, 'tcx> {
        let mut map = FxHashMap::default();
        for row in (1..self.body.local_decls.len()).map(mir::Local::from_usize) {
            if is_copy(cx, self.body.local_decls[row].ty) {
                continue;
            }

            let borrowers = self.possible_borrower.reachable_from(&row);
            if !borrowers.is_empty() {
                let mut bs = HybridBitSet::new_empty(self.body.local_decls.len());
                for &c in borrowers {
                    if c != mir::Local::from_usize(0) {
                        bs.insert(c);
                    }
                }

                if !bs.is_empty() {
                    map.insert(row, bs);
                }
            }
        }

        let bs = BitSet::new_empty(self.body.local_decls.len());
        PossibleBorrowerMap {
            map,
            maybe_live,
            bitset: (bs.clone(), bs),
        }
    }
}

impl<'a, 'tcx> mir::visit::Visitor<'tcx> for PossibleBorrowerVisitor<'a, 'tcx> {
    fn visit_assign(&mut self, place: &mir::Place<'tcx>, rvalue: &mir::Rvalue<'_>, _location: mir::Location) {
        let lhs = place.local;
        match rvalue {
            mir::Rvalue::Ref(_, _, borrowed) => {
                self.possible_borrower.add(borrowed.local, lhs);
            },
            other => {
                if !ContainsRegion.visit_ty(place.ty(&self.body.local_decls, self.cx.tcx).ty) {
                    return;
                }
                rvalue_locals(other, |rhs| {
                    if lhs != rhs {
                        self.possible_borrower.add(rhs, lhs);
                    }
                });
            },
        }
    }

    fn visit_terminator(&mut self, terminator: &mir::Terminator<'_>, _loc: mir::Location) {
        if let mir::TerminatorKind::Call {
            args,
            destination: Some((mir::Place { local: dest, .. }, _)),
            ..
        } = &terminator.kind
        {
            // If the call returns something with lifetimes,
            // let's conservatively assume the returned value contains lifetime of all the arguments.
            // For example, given `let y: Foo<'a> = foo(x)`, `y` is considered to be a possible borrower of `x`.
            if !ContainsRegion.visit_ty(&self.body.local_decls[*dest].ty) {
                return;
            }

            for op in args {
                match op {
                    mir::Operand::Copy(p) | mir::Operand::Move(p) => {
                        self.possible_borrower.add(p.local, *dest);
                    },
                    _ => (),
                }
            }
        }
    }
}

struct ContainsRegion;

impl TypeVisitor<'_> for ContainsRegion {
    fn visit_region(&mut self, _: ty::Region<'_>) -> bool {
        true
    }
}

fn rvalue_locals(rvalue: &mir::Rvalue<'_>, mut visit: impl FnMut(mir::Local)) {
    use rustc_middle::mir::Rvalue::{Aggregate, BinaryOp, Cast, CheckedBinaryOp, Repeat, UnaryOp, Use};

    let mut visit_op = |op: &mir::Operand<'_>| match op {
        mir::Operand::Copy(p) | mir::Operand::Move(p) => visit(p.local),
        _ => (),
    };

    match rvalue {
        Use(op) | Repeat(op, _) | Cast(_, op, _) | UnaryOp(_, op) => visit_op(op),
        Aggregate(_, ops) => ops.iter().for_each(visit_op),
        BinaryOp(_, lhs, rhs) | CheckedBinaryOp(_, lhs, rhs) => {
            visit_op(lhs);
            visit_op(rhs);
        },
        _ => (),
    }
}

/// Result of `PossibleBorrowerVisitor`.
struct PossibleBorrowerMap<'a, 'tcx> {
    /// Mapping `Local -> its possible borrowers`
    map: FxHashMap<mir::Local, HybridBitSet<mir::Local>>,
    maybe_live: ResultsCursor<'a, 'tcx, MaybeStorageLive>,
    // Caches to avoid allocation of `BitSet` on every query
    bitset: (BitSet<mir::Local>, BitSet<mir::Local>),
}

impl PossibleBorrowerMap<'_, '_> {
    /// Returns true if the set of borrowers of `borrowed` living at `at` matches with `borrowers`.
    fn only_borrowers(&mut self, borrowers: &[mir::Local], borrowed: mir::Local, at: mir::Location) -> bool {
        self.maybe_live.seek_after_primary_effect(at);

        self.bitset.0.clear();
        let maybe_live = &mut self.maybe_live;
        if let Some(bitset) = self.map.get(&borrowed) {
            for b in bitset.iter().filter(move |b| maybe_live.contains(*b)) {
                self.bitset.0.insert(b);
            }
        } else {
            return false;
        }

        self.bitset.1.clear();
        for b in borrowers {
            self.bitset.1.insert(*b);
        }

        self.bitset.0 == self.bitset.1
    }
}
