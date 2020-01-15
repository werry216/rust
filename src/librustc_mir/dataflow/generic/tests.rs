//! A test for the logic that updates the state in a `ResultsCursor` during seek.

use rustc::mir::{self, BasicBlock, Location};
use rustc::ty;
use rustc_index::bit_set::BitSet;
use rustc_index::vec::IndexVec;
use rustc_span::DUMMY_SP;

use super::*;
use crate::dataflow::BottomValue;

/// Returns `true` if the given location points to a `Call` terminator that can return
/// successfully.
fn is_call_terminator_non_diverging(body: &mir::Body<'_>, loc: Location) -> bool {
    loc == body.terminator_loc(loc.block)
        && matches!(
            body[loc.block].terminator().kind,
            mir::TerminatorKind::Call { destination: Some(_), ..  }
        )
}

/// Creates a `mir::Body` with a few disconnected basic blocks.
///
/// This is the `Body` that will be used by the `MockAnalysis` below. The shape of its CFG is not
/// important.
fn mock_body() -> mir::Body<'static> {
    let source_info = mir::SourceInfo { scope: mir::OUTERMOST_SOURCE_SCOPE, span: DUMMY_SP };

    let mut blocks = IndexVec::new();
    let mut block = |n, kind| {
        let nop = mir::Statement { source_info, kind: mir::StatementKind::Nop };

        blocks.push(mir::BasicBlockData {
            statements: std::iter::repeat(&nop).cloned().take(n).collect(),
            terminator: Some(mir::Terminator { source_info, kind }),
            is_cleanup: false,
        })
    };

    let dummy_place = mir::Place { local: mir::RETURN_PLACE, projection: ty::List::empty() };

    block(4, mir::TerminatorKind::Return);
    block(1, mir::TerminatorKind::Return);
    block(
        2,
        mir::TerminatorKind::Call {
            func: mir::Operand::Copy(dummy_place.clone()),
            args: vec![],
            destination: Some((dummy_place.clone(), mir::START_BLOCK)),
            cleanup: None,
            from_hir_call: false,
        },
    );
    block(3, mir::TerminatorKind::Return);
    block(0, mir::TerminatorKind::Return);
    block(
        4,
        mir::TerminatorKind::Call {
            func: mir::Operand::Copy(dummy_place.clone()),
            args: vec![],
            destination: Some((dummy_place.clone(), mir::START_BLOCK)),
            cleanup: None,
            from_hir_call: false,
        },
    );

    mir::Body::new_cfg_only(blocks)
}

/// A dataflow analysis whose state is unique at every possible `SeekTarget`.
///
/// Uniqueness is achieved by having a *locally* unique effect before and after each statement and
/// terminator (see `effect_at_target`) while ensuring that the entry set for each block is
/// *globally* unique (see `mock_entry_set`).
///
/// For example, a `BasicBlock` with ID `2` and a `Call` terminator has the following state at each
/// location ("+x" indicates that "x" is added to the state).
///
/// | Location               | Before            | After  |
/// |------------------------|-------------------|--------|
/// | (on_entry)             | {102}                     ||
/// | Statement 0            | +0                | +1     |
/// | statement 1            | +2                | +3     |
/// | `Call` terminator      | +4                | +5     |
/// | (on unwind)            | {102,0,1,2,3,4,5}         ||
/// | (on successful return) | +6                        ||
///
/// The `102` in the block's entry set is derived from the basic block index and ensures that the
/// expected state is unique across all basic blocks. Remember, it is generated by
/// `mock_entry_sets`, not from actually running `MockAnalysis` to fixpoint.
struct MockAnalysis<'tcx> {
    body: &'tcx mir::Body<'tcx>,
}

impl MockAnalysis<'tcx> {
    const BASIC_BLOCK_OFFSET: usize = 100;

    /// The entry set for each `BasicBlock` is the ID of that block offset by a fixed amount to
    /// avoid colliding with the statement/terminator effects.
    fn mock_entry_set(&self, bb: BasicBlock) -> BitSet<usize> {
        let mut ret = BitSet::new_empty(self.bits_per_block(self.body));
        ret.insert(Self::BASIC_BLOCK_OFFSET + bb.index());
        ret
    }

    fn mock_entry_sets(&self) -> IndexVec<BasicBlock, BitSet<usize>> {
        let empty = BitSet::new_empty(self.bits_per_block(self.body));
        let mut ret = IndexVec::from_elem(empty, &self.body.basic_blocks());

        for (bb, _) in self.body.basic_blocks().iter_enumerated() {
            ret[bb] = self.mock_entry_set(bb);
        }

        ret
    }

    /// Returns the index that should be added to the dataflow state at the given target.
    ///
    /// This index is only unique within a given basic block. `SeekAfter` and
    /// `SeekAfterAssumeCallReturns` have the same effect unless `target` is a `Call` terminator.
    fn effect_at_target(&self, target: SeekTarget) -> Option<usize> {
        use SeekTarget::*;

        let idx = match target {
            BlockStart(_) => return None,

            AfterAssumeCallReturns(loc) if is_call_terminator_non_diverging(self.body, loc) => {
                loc.statement_index * 2 + 2
            }

            Before(loc) => loc.statement_index * 2,
            After(loc) | AfterAssumeCallReturns(loc) => loc.statement_index * 2 + 1,
        };

        assert!(idx < Self::BASIC_BLOCK_OFFSET, "Too many statements in basic block");
        Some(idx)
    }

    /// Returns the expected state at the given `SeekTarget`.
    ///
    /// This is the union of index of the target basic block, the index assigned to the
    /// target statement or terminator, and the indices of all preceding statements in the target
    /// basic block.
    ///
    /// For example, the expected state when calling
    /// `seek_before(Location { block: 2, statement_index: 2 })` would be `[102, 0, 1, 2, 3, 4]`.
    fn expected_state_at_target(&self, target: SeekTarget) -> BitSet<usize> {
        let mut ret = BitSet::new_empty(self.bits_per_block(self.body));
        ret.insert(Self::BASIC_BLOCK_OFFSET + target.block().index());

        if let Some(target_effect) = self.effect_at_target(target) {
            for i in 0..=target_effect {
                ret.insert(i);
            }
        }

        ret
    }
}

impl BottomValue for MockAnalysis<'tcx> {
    const BOTTOM_VALUE: bool = false;
}

impl AnalysisDomain<'tcx> for MockAnalysis<'tcx> {
    type Idx = usize;

    const NAME: &'static str = "mock";

    fn bits_per_block(&self, body: &mir::Body<'tcx>) -> usize {
        Self::BASIC_BLOCK_OFFSET + body.basic_blocks().len()
    }

    fn initialize_start_block(&self, _: &mir::Body<'tcx>, _: &mut BitSet<Self::Idx>) {
        unimplemented!("This is never called since `MockAnalysis` is never iterated to fixpoint");
    }
}

impl Analysis<'tcx> for MockAnalysis<'tcx> {
    fn apply_statement_effect(
        &self,
        state: &mut BitSet<Self::Idx>,
        _statement: &mir::Statement<'tcx>,
        location: Location,
    ) {
        let idx = self.effect_at_target(SeekTarget::After(location)).unwrap();
        assert!(state.insert(idx));
    }

    fn apply_before_statement_effect(
        &self,
        state: &mut BitSet<Self::Idx>,
        _statement: &mir::Statement<'tcx>,
        location: Location,
    ) {
        let idx = self.effect_at_target(SeekTarget::Before(location)).unwrap();
        assert!(state.insert(idx));
    }

    fn apply_terminator_effect(
        &self,
        state: &mut BitSet<Self::Idx>,
        _terminator: &mir::Terminator<'tcx>,
        location: Location,
    ) {
        let idx = self.effect_at_target(SeekTarget::After(location)).unwrap();
        assert!(state.insert(idx));
    }

    fn apply_before_terminator_effect(
        &self,
        state: &mut BitSet<Self::Idx>,
        _terminator: &mir::Terminator<'tcx>,
        location: Location,
    ) {
        let idx = self.effect_at_target(SeekTarget::Before(location)).unwrap();
        assert!(state.insert(idx));
    }

    fn apply_call_return_effect(
        &self,
        state: &mut BitSet<Self::Idx>,
        block: BasicBlock,
        _func: &mir::Operand<'tcx>,
        _args: &[mir::Operand<'tcx>],
        _return_place: &mir::Place<'tcx>,
    ) {
        let location = self.body.terminator_loc(block);
        let idx = self.effect_at_target(SeekTarget::AfterAssumeCallReturns(location)).unwrap();
        assert!(state.insert(idx));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SeekTarget {
    BlockStart(BasicBlock),
    Before(Location),
    After(Location),
    AfterAssumeCallReturns(Location),
}

impl SeekTarget {
    fn block(&self) -> BasicBlock {
        use SeekTarget::*;

        match *self {
            BlockStart(block) => block,
            Before(loc) | After(loc) | AfterAssumeCallReturns(loc) => loc.block,
        }
    }

    /// An iterator over all possible `SeekTarget`s in a given block in order, starting with
    /// `BlockStart`.
    ///
    /// This includes both `After` and `AfterAssumeCallReturns` for every `Location`.
    fn iter_in_block(body: &mir::Body<'_>, block: BasicBlock) -> impl Iterator<Item = Self> {
        let statements_and_terminator = (0..=body[block].statements.len())
            .flat_map(|i| (0..3).map(move |j| (i, j)))
            .map(move |(i, kind)| {
                let loc = Location { block, statement_index: i };
                match kind {
                    0 => SeekTarget::Before(loc),
                    1 => SeekTarget::After(loc),
                    2 => SeekTarget::AfterAssumeCallReturns(loc),
                    _ => unreachable!(),
                }
            });

        std::iter::once(SeekTarget::BlockStart(block)).chain(statements_and_terminator)
    }
}

#[test]
fn cursor_seek() {
    let body = mock_body();
    let body = &body;
    let analysis = MockAnalysis { body };

    let mut cursor = Results { entry_sets: analysis.mock_entry_sets(), analysis }.into_cursor(body);

    // Sanity check: the mock call return effect is unique and actually being applied.
    let call_terminator_loc = Location { block: BasicBlock::from_usize(2), statement_index: 2 };
    assert!(is_call_terminator_non_diverging(body, call_terminator_loc));

    let call_return_effect = cursor
        .analysis()
        .effect_at_target(SeekTarget::AfterAssumeCallReturns(call_terminator_loc))
        .unwrap();
    assert_ne!(
        call_return_effect,
        cursor.analysis().effect_at_target(SeekTarget::After(call_terminator_loc)).unwrap()
    );

    cursor.seek_after(call_terminator_loc);
    assert!(!cursor.get().contains(call_return_effect));
    cursor.seek_after_assume_call_returns(call_terminator_loc);
    assert!(cursor.get().contains(call_return_effect));

    let every_target = || {
        body.basic_blocks()
            .iter_enumerated()
            .flat_map(|(bb, _)| SeekTarget::iter_in_block(body, bb))
    };

    let mut seek_to_target = |targ| {
        use SeekTarget::*;

        match targ {
            BlockStart(block) => cursor.seek_to_block_start(block),
            Before(loc) => cursor.seek_before(loc),
            After(loc) => cursor.seek_after(loc),
            AfterAssumeCallReturns(loc) => cursor.seek_after_assume_call_returns(loc),
        }

        assert_eq!(cursor.get(), &cursor.analysis().expected_state_at_target(targ));
    };

    // Seek *to* every possible `SeekTarget` *from* every possible `SeekTarget`.
    //
    // By resetting the cursor to `from` each time it changes, we end up checking some edges twice.
    // What we really want is an Eulerian cycle for the complete digraph over all possible
    // `SeekTarget`s, but it's not worth spending the time to compute it.
    for from in every_target() {
        seek_to_target(from);

        for to in every_target() {
            seek_to_target(to);
            seek_to_target(from);
        }
    }
}
