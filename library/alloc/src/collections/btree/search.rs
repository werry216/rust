use core::borrow::Borrow;
use core::cmp::Ordering;
use core::ops::{Bound, RangeBounds};

use super::node::{marker, ForceResult::*, Handle, NodeRef};

use SearchBound::*;
use SearchResult::*;

pub enum SearchBound<T> {
    /// An inclusive bound to look for, just like `Bound::Included(T)`.
    Included(T),
    /// An exclusive bound to look for, just like `Bound::Excluded(T)`.
    Excluded(T),
    /// An unconditional inclusive bound, just like `Bound::Unbounded`.
    AllIncluded,
    /// An unconditional exclusive bound.
    AllExcluded,
}

impl<T> SearchBound<T> {
    pub fn from_range(range_bound: Bound<T>) -> Self {
        match range_bound {
            Bound::Included(t) => Included(t),
            Bound::Excluded(t) => Excluded(t),
            Bound::Unbounded => AllIncluded,
        }
    }
}

pub enum SearchResult<BorrowType, K, V, FoundType, GoDownType> {
    Found(Handle<NodeRef<BorrowType, K, V, FoundType>, marker::KV>),
    GoDown(Handle<NodeRef<BorrowType, K, V, GoDownType>, marker::Edge>),
}

pub enum IndexResult {
    KV(usize),
    Edge(usize),
}

impl<BorrowType: marker::BorrowType, K, V> NodeRef<BorrowType, K, V, marker::LeafOrInternal> {
    /// Looks up a given key in a (sub)tree headed by the node, recursively.
    /// Returns a `Found` with the handle of the matching KV, if any. Otherwise,
    /// returns a `GoDown` with the handle of the leaf edge where the key belongs.
    ///
    /// The result is meaningful only if the tree is ordered by key, like the tree
    /// in a `BTreeMap` is.
    pub fn search_tree<Q: ?Sized>(
        mut self,
        key: &Q,
    ) -> SearchResult<BorrowType, K, V, marker::LeafOrInternal, marker::Leaf>
    where
        Q: Ord,
        K: Borrow<Q>,
    {
        loop {
            self = match self.search_node(key) {
                Found(handle) => return Found(handle),
                GoDown(handle) => match handle.force() {
                    Leaf(leaf) => return GoDown(leaf),
                    Internal(internal) => internal.descend(),
                },
            }
        }
    }

    /// Descends to the nearest node where the edge matching the lower bound
    /// of the range is different from the edge matching the upper bound, i.e.,
    /// the nearest node that has at least one key contained in the range.
    ///
    /// If found, returns an `Ok` with that node, the pair of edge indices in it
    /// delimiting the range, and the corresponding pair of bounds for
    /// continuing the search in the child nodes, in case the node is internal.
    ///
    /// If not found, returns an `Err` with the leaf edge matching the entire
    /// range.
    ///
    /// The result is meaningful only if the tree is ordered by key.
    pub fn search_tree_for_bifurcation<'r, Q: ?Sized, R>(
        mut self,
        range: &'r R,
    ) -> Result<
        (
            NodeRef<BorrowType, K, V, marker::LeafOrInternal>,
            usize,
            usize,
            SearchBound<&'r Q>,
            SearchBound<&'r Q>,
        ),
        Handle<NodeRef<BorrowType, K, V, marker::Leaf>, marker::Edge>,
    >
    where
        Q: Ord,
        K: Borrow<Q>,
        R: RangeBounds<Q>,
    {
        // It might be unsound to inline these variables if this logic changes (#81138).
        // We assume the bounds reported by `range` remain the same, but
        // an adversarial implementation could change between calls
        let (start, end) = (range.start_bound(), range.end_bound());
        match (start, end) {
            (Bound::Excluded(s), Bound::Excluded(e)) if s == e => {
                panic!("range start and end are equal and excluded in BTreeMap")
            }
            (Bound::Included(s) | Bound::Excluded(s), Bound::Included(e) | Bound::Excluded(e))
                if s > e =>
            {
                panic!("range start is greater than range end in BTreeMap")
            }
            _ => {}
        }
        let mut lower_bound = SearchBound::from_range(start);
        let mut upper_bound = SearchBound::from_range(end);
        loop {
            let (lower_edge_idx, lower_child_bound) = self.find_lower_bound_index(lower_bound);
            let (upper_edge_idx, upper_child_bound) = self.find_upper_bound_index(upper_bound);
            // SAFETY: This panic is used for safety, so external impls can't be called here. The
            // comparison is done with integers for that reason.
            if lower_edge_idx > upper_edge_idx {
                panic!("Ord is ill-defined in BTreeMap range")
            }
            if lower_edge_idx < upper_edge_idx {
                return Ok((
                    self,
                    lower_edge_idx,
                    upper_edge_idx,
                    lower_child_bound,
                    upper_child_bound,
                ));
            }
            let common_edge = unsafe { Handle::new_edge(self, lower_edge_idx) };
            match common_edge.force() {
                Leaf(common_edge) => return Err(common_edge),
                Internal(common_edge) => {
                    self = common_edge.descend();
                    lower_bound = lower_child_bound;
                    upper_bound = upper_child_bound;
                }
            }
        }
    }

    /// Finds an edge in the node delimiting the lower bound of a range.
    /// Also returns the lower bound to be used for continuing the search in
    /// the matching child node, if `self` is an internal node.
    ///
    /// The result is meaningful only if the tree is ordered by key.
    pub fn find_lower_bound_edge<'r, Q>(
        self,
        bound: SearchBound<&'r Q>,
    ) -> (Handle<Self, marker::Edge>, SearchBound<&'r Q>)
    where
        Q: ?Sized + Ord,
        K: Borrow<Q>,
    {
        let (edge_idx, bound) = self.find_lower_bound_index(bound);
        let edge = unsafe { Handle::new_edge(self, edge_idx) };
        (edge, bound)
    }

    /// Clone of `find_lower_bound_edge` for the upper bound.
    pub fn find_upper_bound_edge<'r, Q>(
        self,
        bound: SearchBound<&'r Q>,
    ) -> (Handle<Self, marker::Edge>, SearchBound<&'r Q>)
    where
        Q: ?Sized + Ord,
        K: Borrow<Q>,
    {
        let (edge_idx, bound) = self.find_upper_bound_index(bound);
        let edge = unsafe { Handle::new_edge(self, edge_idx) };
        (edge, bound)
    }
}

impl<BorrowType, K, V, Type> NodeRef<BorrowType, K, V, Type> {
    /// Looks up a given key in the node, without recursion.
    /// Returns a `Found` with the handle of the matching KV, if any. Otherwise,
    /// returns a `GoDown` with the handle of the edge where the key might be found
    /// (if the node is internal) or where the key can be inserted.
    ///
    /// The result is meaningful only if the tree is ordered by key, like the tree
    /// in a `BTreeMap` is.
    pub fn search_node<Q: ?Sized>(self, key: &Q) -> SearchResult<BorrowType, K, V, Type, Type>
    where
        Q: Ord,
        K: Borrow<Q>,
    {
        match self.find_key_index(key) {
            IndexResult::KV(idx) => Found(unsafe { Handle::new_kv(self, idx) }),
            IndexResult::Edge(idx) => GoDown(unsafe { Handle::new_edge(self, idx) }),
        }
    }

    /// Returns either the KV index in the node at which the key (or an equivalent)
    /// exists, or the edge index where the key belongs.
    ///
    /// The result is meaningful only if the tree is ordered by key, like the tree
    /// in a `BTreeMap` is.
    fn find_key_index<Q: ?Sized>(&self, key: &Q) -> IndexResult
    where
        Q: Ord,
        K: Borrow<Q>,
    {
        let node = self.reborrow();
        let keys = node.keys();
        for (i, k) in keys.iter().enumerate() {
            match key.cmp(k.borrow()) {
                Ordering::Greater => {}
                Ordering::Equal => return IndexResult::KV(i),
                Ordering::Less => return IndexResult::Edge(i),
            }
        }
        IndexResult::Edge(keys.len())
    }

    /// Finds an edge index in the node delimiting the lower bound of a range.
    /// Also returns the lower bound to be used for continuing the search in
    /// the matching child node, if `self` is an internal node.
    ///
    /// The result is meaningful only if the tree is ordered by key.
    fn find_lower_bound_index<'r, Q>(
        &self,
        bound: SearchBound<&'r Q>,
    ) -> (usize, SearchBound<&'r Q>)
    where
        Q: ?Sized + Ord,
        K: Borrow<Q>,
    {
        match bound {
            Included(key) => match self.find_key_index(key) {
                IndexResult::KV(idx) => (idx, AllExcluded),
                IndexResult::Edge(idx) => (idx, bound),
            },
            Excluded(key) => match self.find_key_index(key) {
                IndexResult::KV(idx) => (idx + 1, AllIncluded),
                IndexResult::Edge(idx) => (idx, bound),
            },
            AllIncluded => (0, AllIncluded),
            AllExcluded => (self.len(), AllExcluded),
        }
    }

    /// Clone of `find_lower_bound_index` for the upper bound.
    fn find_upper_bound_index<'r, Q>(
        &self,
        bound: SearchBound<&'r Q>,
    ) -> (usize, SearchBound<&'r Q>)
    where
        Q: ?Sized + Ord,
        K: Borrow<Q>,
    {
        match bound {
            Included(key) => match self.find_key_index(key) {
                IndexResult::KV(idx) => (idx + 1, AllExcluded),
                IndexResult::Edge(idx) => (idx, bound),
            },
            Excluded(key) => match self.find_key_index(key) {
                IndexResult::KV(idx) => (idx, AllIncluded),
                IndexResult::Edge(idx) => (idx, bound),
            },
            AllIncluded => (self.len(), AllIncluded),
            AllExcluded => (0, AllExcluded),
        }
    }
}
