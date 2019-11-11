//! A framework for expressing dataflow problems.

use std::io;

use rustc::mir::{self, BasicBlock, Location};
use rustc_index::bit_set::{BitSet, HybridBitSet};
use rustc_index::vec::{Idx, IndexVec};

use crate::dataflow::BottomValue;

mod cursor;
mod engine;
mod graphviz;

pub use self::cursor::{ResultsCursor, ResultsRefCursor};
pub use self::engine::Engine;

/// A dataflow analysis that has converged to fixpoint.
pub struct Results<'tcx, A>
where
    A: Analysis<'tcx>,
{
    pub analysis: A,
    entry_sets: IndexVec<BasicBlock, BitSet<A::Idx>>,
}

impl<A> Results<'tcx, A>
where
    A: Analysis<'tcx>,
{
    pub fn into_cursor(self, body: &'mir mir::Body<'tcx>) -> ResultsCursor<'mir, 'tcx, A> {
        ResultsCursor::new(body, self)
    }

    pub fn on_block_entry(&self, block: BasicBlock) -> &BitSet<A::Idx> {
        &self.entry_sets[block]
    }
}

/// Define the domain of a dataflow problem.
///
/// This trait specifies the lattice on which this analysis operates. For now, this must be a
/// powerset of values of type `Idx`. The elements of this lattice are represented with a `BitSet`
/// and referred to as the state vector.
///
/// This trait also defines the initial value for the dataflow state upon entry to the
/// `START_BLOCK`, as well as some names used to refer to this analysis when debugging.
pub trait AnalysisDomain<'tcx>: BottomValue {
    /// The type of the elements in the state vector.
    type Idx: Idx;

    /// A descriptive name for this analysis. Used only for debugging.
    ///
    /// This name should be brief and contain no spaces, periods or other characters that are not
    /// suitable as part of a filename.
    const NAME: &'static str;

    /// The size of the state vector.
    fn bits_per_block(&self, body: &mir::Body<'tcx>) -> usize;

    /// Mutates the entry set of the `START_BLOCK` to contain the initial state for dataflow
    /// analysis.
    fn initialize_start_block(&self, body: &mir::Body<'tcx>, state: &mut BitSet<Self::Idx>);

    /// Prints an element in the state vector for debugging.
    fn pretty_print_idx(&self, w: &mut impl io::Write, idx: Self::Idx) -> io::Result<()> {
        write!(w, "{:?}", idx)
    }
}

/// Define a dataflow problem with an arbitrarily complex transfer function.
pub trait Analysis<'tcx>: AnalysisDomain<'tcx> {
    /// Updates the current dataflow state with the effect of evaluating a statement.
    fn apply_statement_effect(
        &self,
        state: &mut BitSet<Self::Idx>,
        statement: &mir::Statement<'tcx>,
        location: Location,
    );

    /// Updates the current dataflow state with an effect that occurs immediately *before* the
    /// given statement.
    ///
    /// This method is useful if the consumer of the results of this analysis needs only to observe
    /// *part* of the effect of a statement (e.g. for two-phase borrows). As a general rule,
    /// analyses should not implement this without implementing `apply_statement_effect`.
    fn apply_before_statement_effect(
        &self,
        _state: &mut BitSet<Self::Idx>,
        _statement: &mir::Statement<'tcx>,
        _location: Location,
    ) {
    }

    /// Updates the current dataflow state with the effect of evaluating a terminator.
    ///
    /// The effect of a successful return from a `Call` terminator should **not** be accounted for
    /// in this function. That should go in `apply_call_return_effect`. For example, in the
    /// `InitializedPlaces` analyses, the return place for a function call is not marked as
    /// initialized here.
    fn apply_terminator_effect(
        &self,
        state: &mut BitSet<Self::Idx>,
        terminator: &mir::Terminator<'tcx>,
        location: Location,
    );

    /// Updates the current dataflow state with an effect that occurs immediately *before* the
    /// given terminator.
    ///
    /// This method is useful if the consumer of the results of this analysis needs only to observe
    /// *part* of the effect of a terminator (e.g. for two-phase borrows). As a general rule,
    /// analyses should not implement this without implementing `apply_terminator_effect`.
    fn apply_before_terminator_effect(
        &self,
        _state: &mut BitSet<Self::Idx>,
        _terminator: &mir::Terminator<'tcx>,
        _location: Location,
    ) {
    }

    /// Updates the current dataflow state with the effect of a successful return from a `Call`
    /// terminator.
    ///
    /// This is separate from `apply_terminator_effect` to properly track state across unwind
    /// edges.
    fn apply_call_return_effect(
        &self,
        state: &mut BitSet<Self::Idx>,
        block: BasicBlock,
        func: &mir::Operand<'tcx>,
        args: &[mir::Operand<'tcx>],
        return_place: &mir::Place<'tcx>,
    );
}

/// Define a gen/kill dataflow problem.
///
/// Each method in this trait has a corresponding one in `Analysis`. However, these methods only
/// allow modification of the dataflow state via "gen" and "kill" operations. By defining transfer
/// functions for each statement in this way, the transfer function for an entire basic block can
/// be computed efficiently.
///
/// `Analysis` is automatically implemented for all implementers of `GenKillAnalysis`.
pub trait GenKillAnalysis<'tcx>: Analysis<'tcx> {
    /// See `Analysis::apply_statement_effect`.
    fn statement_effect(
        &self,
        trans: &mut impl GenKill<Self::Idx>,
        statement: &mir::Statement<'tcx>,
        location: Location,
    );

    /// See `Analysis::apply_before_statement_effect`.
    fn before_statement_effect(
        &self,
        _trans: &mut impl GenKill<Self::Idx>,
        _statement: &mir::Statement<'tcx>,
        _location: Location,
    ) {
    }

    /// See `Analysis::apply_terminator_effect`.
    fn terminator_effect(
        &self,
        trans: &mut impl GenKill<Self::Idx>,
        terminator: &mir::Terminator<'tcx>,
        location: Location,
    );

    /// See `Analysis::apply_before_terminator_effect`.
    fn before_terminator_effect(
        &self,
        _trans: &mut impl GenKill<Self::Idx>,
        _terminator: &mir::Terminator<'tcx>,
        _location: Location,
    ) {
    }

    /// See `Analysis::apply_call_return_effect`.
    fn call_return_effect(
        &self,
        trans: &mut impl GenKill<Self::Idx>,
        block: BasicBlock,
        func: &mir::Operand<'tcx>,
        args: &[mir::Operand<'tcx>],
        return_place: &mir::Place<'tcx>,
    );
}

impl<A> Analysis<'tcx> for A
where
    A: GenKillAnalysis<'tcx>,
{
    fn apply_statement_effect(
        &self,
        state: &mut BitSet<Self::Idx>,
        statement: &mir::Statement<'tcx>,
        location: Location,
    ) {
        self.statement_effect(state, statement, location);
    }

    fn apply_before_statement_effect(
        &self,
        state: &mut BitSet<Self::Idx>,
        statement: &mir::Statement<'tcx>,
        location: Location,
    ) {
        self.before_statement_effect(state, statement, location);
    }

    fn apply_terminator_effect(
        &self,
        state: &mut BitSet<Self::Idx>,
        terminator: &mir::Terminator<'tcx>,
        location: Location,
    ) {
        self.terminator_effect(state, terminator, location);
    }

    fn apply_before_terminator_effect(
        &self,
        state: &mut BitSet<Self::Idx>,
        terminator: &mir::Terminator<'tcx>,
        location: Location,
    ) {
        self.before_terminator_effect(state, terminator, location);
    }

    fn apply_call_return_effect(
        &self,
        state: &mut BitSet<Self::Idx>,
        block: BasicBlock,
        func: &mir::Operand<'tcx>,
        args: &[mir::Operand<'tcx>],
        return_place: &mir::Place<'tcx>,
    ) {
        self.call_return_effect(state, block, func, args, return_place);
    }
}

/// The legal operations for a transfer function in a gen/kill problem.
pub trait GenKill<T>: Sized {
    /// Inserts `elem` into the `gen` set, removing it the `kill` set if present.
    fn gen(&mut self, elem: T);

    /// Inserts `elem` into the `kill` set, removing it the `gen` set if present.
    fn kill(&mut self, elem: T);

    /// Inserts the given elements into the `gen` set, removing them from the `kill` set if present.
    fn gen_all(&mut self, elems: impl IntoIterator<Item = T>) {
        for elem in elems {
            self.gen(elem);
        }
    }

    /// Inserts the given elements into the `kill` set, removing them from the `gen` set if present.
    fn kill_all(&mut self, elems: impl IntoIterator<Item = T>) {
        for elem in elems {
            self.kill(elem);
        }
    }
}

/// Stores a transfer function for a gen/kill problem.
#[derive(Clone)]
pub struct GenKillSet<T: Idx> {
    gen: HybridBitSet<T>,
    kill: HybridBitSet<T>,
}

impl<T: Idx> GenKillSet<T> {
    /// Creates a new transfer function that will leave the dataflow state unchanged.
    pub fn identity(universe: usize) -> Self {
        GenKillSet {
            gen: HybridBitSet::new_empty(universe),
            kill: HybridBitSet::new_empty(universe),
        }
    }

    /// Applies this transfer function to the given bitset.
    pub fn apply(&self, state: &mut BitSet<T>) {
        state.union(&self.gen);
        state.subtract(&self.kill);
    }
}

impl<T: Idx> GenKill<T> for GenKillSet<T> {
    fn gen(&mut self, elem: T) {
        self.gen.insert(elem);
        self.kill.remove(elem);
    }

    fn kill(&mut self, elem: T) {
        self.kill.insert(elem);
        self.gen.remove(elem);
    }
}

impl<T: Idx> GenKill<T> for BitSet<T> {
    fn gen(&mut self, elem: T) {
        self.insert(elem);
    }

    fn kill(&mut self, elem: T) {
        self.remove(elem);
    }
}
