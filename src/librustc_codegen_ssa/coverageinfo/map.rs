use rustc_middle::ty::Instance;
use rustc_middle::ty::TyCtxt;
use rustc_span::source_map::{Pos, SourceMap};
use rustc_span::{BytePos, FileName, Loc, RealFileName};

use std::cmp::{Ord, Ordering};
use std::fmt;
use std::path::PathBuf;

/// Aligns with [llvm::coverage::Counter::CounterKind](https://github.com/rust-lang/llvm-project/blob/rustc/10.0-2020-05-05/llvm/include/llvm/ProfileData/Coverage/CoverageMapping.h#L91)
#[derive(Copy, Clone, Debug)]
#[repr(C)]
enum CounterKind {
    Zero = 0,
    CounterValueReference = 1,
    Expression = 2,
}

/// A reference to an instance of an abstract "counter" that will yield a value in a coverage
/// report. Note that `id` has different interpretations, depending on the `kind`:
///   * For `CounterKind::Zero`, `id` is assumed to be `0`
///   * For `CounterKind::CounterValueReference`,  `id` matches the `counter_id` of the injected
///     instrumentation counter (the `index` argument to the LLVM intrinsic `instrprof.increment()`)
///   * For `CounterKind::Expression`, `id` is the index into the coverage map's array of counter
///     expressions.
/// Aligns with [llvm::coverage::Counter](https://github.com/rust-lang/llvm-project/blob/rustc/10.0-2020-05-05/llvm/include/llvm/ProfileData/Coverage/CoverageMapping.h#L98-L99)
/// Important: The Rust struct layout (order and types of fields) must match its C++ counterpart.
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct Counter {
    // Important: The layout (order and types of fields) must match its C++ counterpart.
    kind: CounterKind,
    id: u32,
}

impl Counter {
    pub fn zero() -> Self {
        Self { kind: CounterKind::Zero, id: 0 }
    }

    pub fn counter_value_reference(counter_id: u32) -> Self {
        Self { kind: CounterKind::CounterValueReference, id: counter_id }
    }

    pub fn expression(final_expression_index: u32) -> Self {
        Self { kind: CounterKind::Expression, id: final_expression_index }
    }
}

/// Aligns with [llvm::coverage::CounterExpression::ExprKind](https://github.com/rust-lang/llvm-project/blob/rustc/10.0-2020-05-05/llvm/include/llvm/ProfileData/Coverage/CoverageMapping.h#L146)
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub enum ExprKind {
    Subtract = 0,
    Add = 1,
}

/// Aligns with [llvm::coverage::CounterExpression](https://github.com/rust-lang/llvm-project/blob/rustc/10.0-2020-05-05/llvm/include/llvm/ProfileData/Coverage/CoverageMapping.h#L147-L148)
/// Important: The Rust struct layout (order and types of fields) must match its C++ counterpart.
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct CounterExpression {
    kind: ExprKind,
    lhs: Counter,
    rhs: Counter,
}

impl CounterExpression {
    pub fn new(lhs: Counter, kind: ExprKind, rhs: Counter) -> Self {
        Self { kind, lhs, rhs }
    }
}

#[derive(Clone, Debug)]
pub struct Region {
    start: Loc,
    end: Loc,
}

impl Ord for Region {
    fn cmp(&self, other: &Self) -> Ordering {
        (&self.start.file.name, &self.start.line, &self.start.col, &self.end.line, &self.end.col)
            .cmp(&(
                &other.start.file.name,
                &other.start.line,
                &other.start.col,
                &other.end.line,
                &other.end.col,
            ))
    }
}

impl PartialOrd for Region {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Region {
    fn eq(&self, other: &Self) -> bool {
        self.start.file.name == other.start.file.name
            && self.start.line == other.start.line
            && self.start.col == other.start.col
            && self.end.line == other.end.line
            && self.end.col == other.end.col
    }
}

impl Eq for Region {}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (file_path, start_line, start_col, end_line, end_col) = self.file_start_and_end();
        write!(f, "{:?}:{}:{} - {}:{}", file_path, start_line, start_col, end_line, end_col)
    }
}

impl Region {
    pub fn new(source_map: &SourceMap, start_byte_pos: u32, end_byte_pos: u32) -> Self {
        let start = source_map.lookup_char_pos(BytePos::from_u32(start_byte_pos));
        let end = source_map.lookup_char_pos(BytePos::from_u32(end_byte_pos));
        assert_eq!(start.file.name, end.file.name);
        Self { start, end }
    }

    pub fn file_start_and_end<'a>(&'a self) -> (&'a PathBuf, u32, u32, u32, u32) {
        let start = &self.start;
        let end = &self.end;
        match &start.file.name {
            FileName::Real(RealFileName::Named(path)) => (
                path,
                start.line as u32,
                start.col.to_u32() + 1,
                end.line as u32,
                end.col.to_u32() + 1,
            ),
            _ => {
                bug!("start.file.name should be a RealFileName, but it was: {:?}", start.file.name)
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExpressionRegion {
    lhs: u32,
    op: ExprKind,
    rhs: u32,
    region: Region,
}

// FIXME(richkadel): There seems to be a problem computing the file location in
// some cases. I need to investigate this more. When I generate and show coverage
// for the example binary in the crates.io crate `json5format`, I had a couple of
// notable problems:
//
//   1. I saw a lot of coverage spans in `llvm-cov show` highlighting regions in
//      various comments (not corresponding to rustdoc code), indicating a possible
//      problem with the byte_pos-to-source-map implementation.
//
//   2. And (perhaps not related) when I build the aforementioned example binary with:
//      `RUST_FLAGS="-Zinstrument-coverage" cargo build --example formatjson5`
//      and then run that binary with
//      `LLVM_PROFILE_FILE="formatjson5.profraw" ./target/debug/examples/formatjson5 \
//      some.json5` for some reason the binary generates *TWO* `.profraw` files. One
//      named `default.profraw` and the other named `formatjson5.profraw` (the expected
//      name, in this case).
//
//   3. I think that if I eliminate regions within a function, their region_ids,
//      referenced in expressions, will be wrong? I think the ids are implied by their
//      array position in the final coverage map output (IIRC).
//
//   4. I suspect a problem (if not the only problem) is the SourceMap is wrong for some
//      region start/end byte positions. Just like I couldn't get the function hash at
//      intrinsic codegen time for external crate functions, I think the SourceMap I
//      have here only applies to the local crate, and I know I have coverages that
//      reference external crates.
//
//          I still don't know if I fixed the hash problem correctly. If external crates
//          implement the function, can't I use the coverage counters already compiled
//          into those external crates? (Maybe not for generics and/or maybe not for
//          macros... not sure. But I need to understand this better.)
//
// If the byte range conversion is wrong, fix it. But if it
// is right, then it is possible for the start and end to be in different files.
// Can I do something other than ignore coverages that span multiple files?
//
// If I can resolve this, remove the "Option<>" result type wrapper
// `regions_in_file_order()` accordingly.

/// Collects all of the coverage regions associated with (a) injected counters, (b) counter
/// expressions (additions or subtraction), and (c) unreachable regions (always counted as zero),
/// for a given Function. Counters and counter expressions have non-overlapping `id`s because they
/// can both be operands in an expression. This struct also stores the `function_source_hash`,
/// computed during instrumentation, and forwarded with counters.
///
/// Note, it may be important to understand LLVM's definitions of `unreachable` regions versus "gap
/// regions" (or "gap areas"). A gap region is a code region within a counted region (either counter
/// or expression), but the line or lines in the gap region are not executable (such as lines with
/// only whitespace or comments). According to LLVM Code Coverage Mapping documentation, "A count
/// for a gap area is only used as the line execution count if there are no other regions on a
/// line."
pub struct FunctionCoverage<'a> {
    source_map: &'a SourceMap,
    source_hash: u64,
    counters: Vec<Option<Region>>,
    expressions: Vec<Option<ExpressionRegion>>,
    unreachable_regions: Vec<Region>,
}

impl<'a> FunctionCoverage<'a> {
    pub fn new<'tcx: 'a>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Self {
        let coverageinfo = tcx.coverageinfo(instance.def_id());
        Self {
            source_map: tcx.sess.source_map(),
            source_hash: 0, // will be set with the first `add_counter()`
            counters: vec![None; coverageinfo.num_counters as usize],
            expressions: vec![None; coverageinfo.num_expressions as usize],
            unreachable_regions: Vec::new(),
        }
    }

    /// Adds a code region to be counted by an injected counter intrinsic.
    /// The source_hash (computed during coverage instrumentation) should also be provided, and
    /// should be the same for all counters in a given function.
    pub fn add_counter(
        &mut self,
        source_hash: u64,
        id: u32,
        start_byte_pos: u32,
        end_byte_pos: u32,
    ) {
        if self.source_hash == 0 {
            self.source_hash = source_hash;
        } else {
            debug_assert_eq!(source_hash, self.source_hash);
        }
        self.counters[id as usize]
            .replace(Region::new(self.source_map, start_byte_pos, end_byte_pos))
            .expect_none("add_counter called with duplicate `id`");
    }

    /// Both counters and "counter expressions" (or simply, "expressions") can be operands in other
    /// expressions. Expression IDs start from `u32::MAX` and go down, so the range of expression
    /// IDs will not overlap with the range of counter IDs. Counters and expressions can be added in
    /// any order, and expressions can still be assigned contiguous (though descending) IDs, without
    /// knowing what the last counter ID will be.
    ///
    /// When storing the expression data in the `expressions` vector in the `FunctionCoverage`
    /// struct, its vector index is computed, from the given expression ID, by subtracting from
    /// `u32::MAX`.
    ///
    /// Since the expression operands (`lhs` and `rhs`) can reference either counters or
    /// expressions, an operand that references an expression also uses its original ID, descending
    /// from `u32::MAX`. Theses operands are translated only during code generation, after all
    /// counters and expressions have been added.
    pub fn add_counter_expression(
        &mut self,
        id_descending_from_max: u32,
        lhs: u32,
        op: ExprKind,
        rhs: u32,
        start_byte_pos: u32,
        end_byte_pos: u32,
    ) {
        let expression_index = self.expression_index(id_descending_from_max);
        self.expressions[expression_index]
            .replace(ExpressionRegion {
                lhs,
                op,
                rhs,
                region: Region::new(self.source_map, start_byte_pos, end_byte_pos),
            })
            .expect_none("add_counter_expression called with duplicate `id_descending_from_max`");
    }

    /// Add a region that will be marked as "unreachable", with a constant "zero counter".
    pub fn add_unreachable_region(&mut self, start_byte_pos: u32, end_byte_pos: u32) {
        self.unreachable_regions.push(Region::new(self.source_map, start_byte_pos, end_byte_pos));
    }

    /// Return the source hash, generated from the HIR node structure, and used to indicate whether
    /// or not the source code structure changed between different compilations.
    pub fn source_hash(&self) -> u64 {
        self.source_hash
    }

    /// Generate an array of CounterExpressions, and an iterator over all `Counter`s and their
    /// associated `Regions` (from which the LLVM-specific `CoverageMapGenerator` will create
    /// `CounterMappingRegion`s.
    pub fn get_expressions_and_counter_regions(
        &'a self,
    ) -> (Vec<CounterExpression>, impl Iterator<Item = (Counter, &'a Region)>) {
        assert!(self.source_hash != 0);

        let counter_regions = self.counter_regions();
        let (expressions, expression_regions) = self.expressions_with_regions();
        let unreachable_regions = self.unreachable_regions();

        let counter_regions =
            counter_regions.chain(expression_regions.into_iter().chain(unreachable_regions));
        (expressions, counter_regions)
    }

    fn counter_regions(&'a self) -> impl Iterator<Item = (Counter, &'a Region)> {
        self.counters.iter().enumerate().filter_map(|(index, entry)| {
            // Option::map() will return None to filter out missing counters. This may happen
            // if, for example, a MIR-instrumented counter is removed during an optimization.
            entry.as_ref().map(|region| (Counter::counter_value_reference(index as u32), region))
        })
    }

    fn expressions_with_regions(
        &'a self,
    ) -> (Vec<CounterExpression>, impl Iterator<Item = (Counter, &'a Region)>) {
        let mut counter_expressions = Vec::with_capacity(self.expressions.len());
        let mut expression_regions = Vec::with_capacity(self.expressions.len());
        let mut new_indexes = vec![u32::MAX; self.expressions.len()];

        // Note that an `ExpressionRegion`s at any given index can include other expressions as
        // operands, but expression operands can only come from the subset of expressions having
        // `expression_index`s lower than the referencing `ExpressionRegion`. Therefore, it is
        // reasonable to look up the new index of an expression operand while the `new_indexes`
        // vector is only complete up to the current `ExpressionIndex`.
        let id_to_counter = |new_indexes: &Vec<u32>, id| {
            if id < self.counters.len() as u32 {
                self.counters
                    .get(id as usize)
                    .expect("id is out of range")
                    .as_ref()
                    .map(|_| Counter::counter_value_reference(id))
            } else {
                let index = self.expression_index(id);
                self.expressions
                    .get(index)
                    .expect("id is out of range")
                    .as_ref()
                    .map(|_| Counter::expression(new_indexes[index]))
            }
        };

        for (original_index, expression_region) in
            self.expressions.iter().enumerate().filter_map(|(original_index, entry)| {
                // Option::map() will return None to filter out missing expressions. This may happen
                // if, for example, a MIR-instrumented expression is removed during an optimization.
                entry.as_ref().map(|region| (original_index, region))
            })
        {
            let region = &expression_region.region;
            let ExpressionRegion { lhs, op, rhs, .. } = *expression_region;

            if let Some(Some((lhs_counter, rhs_counter))) =
                id_to_counter(&new_indexes, lhs).map(|lhs_counter| {
                    id_to_counter(&new_indexes, rhs).map(|rhs_counter| (lhs_counter, rhs_counter))
                })
            {
                // Both operands exist. `Expression` operands exist in `self.expressions` and have
                // been assigned a `new_index`.
                let final_expression_index = counter_expressions.len() as u32;
                counter_expressions.push(CounterExpression::new(lhs_counter, op, rhs_counter));
                new_indexes[original_index] = final_expression_index;
                expression_regions.push((Counter::expression(final_expression_index), region));
            }
        }
        (counter_expressions, expression_regions.into_iter())
    }

    fn unreachable_regions(&'a self) -> impl Iterator<Item = (Counter, &'a Region)> {
        self.unreachable_regions.iter().map(|region| (Counter::zero(), region))
    }

    fn expression_index(&self, id_descending_from_max: u32) -> usize {
        debug_assert!(id_descending_from_max as usize >= self.counters.len());
        (u32::MAX - id_descending_from_max) as usize
    }
}
