//! This is a reproducer for the ICE 6840: https://github.com/rust-lang/rust-clippy/issues/6840.
//! The ICE is caused by `TyCtxt::layout_of` and `is_normalizable` not being strict enough
#![allow(dead_code)]
use std::collections::HashMap;

pub trait Rule {
    type DependencyKey;
}

pub struct RuleEdges<R: Rule> {
    dependencies: R::DependencyKey,
}

type RuleDependencyEdges<R> = HashMap<u32, RuleEdges<R>>;

// and additional potential variants
type RuleDependencyEdgesArray<R> = HashMap<u32, [RuleEdges<R>; 8]>;
type RuleDependencyEdgesSlice<R> = HashMap<u32, &'static [RuleEdges<R>]>;
type RuleDependencyEdgesRef<R> = HashMap<u32, &'static RuleEdges<R>>;
type RuleDependencyEdgesRaw<R> = HashMap<u32, *const RuleEdges<R>>;
type RuleDependencyEdgesTuple<R> = HashMap<u32, (RuleEdges<R>, RuleEdges<R>)>;

fn main() {}
