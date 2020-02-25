use crate::fx::FxHashMap;
use crate::undo_log::{Rollback, Snapshots, UndoLogs, VecLog};
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops;

pub use crate::undo_log::Snapshot;

#[cfg(test)]
mod tests;

pub type SnapshotMapStorage<K, V> = SnapshotMap<K, V, FxHashMap<K, V>, ()>;

pub struct SnapshotMap<K, V, M = FxHashMap<K, V>, L = VecLog<UndoLog<K, V>>> {
    map: M,
    undo_log: L,
    _marker: PhantomData<(K, V)>,
}

// HACK(eddyb) manual impl avoids `Default` bounds on `K` and `V`.
impl<K, V, M, L> Default for SnapshotMap<K, V, M, L>
where
    M: Default,
    L: Default,
{
    fn default() -> Self {
        SnapshotMap { map: Default::default(), undo_log: Default::default(), _marker: PhantomData }
    }
}

pub enum UndoLog<K, V> {
    Inserted(K),
    Overwrite(K, V),
    Purged,
}

impl<K, V, M, L> SnapshotMap<K, V, M, L> {
    pub fn with_log<L2>(&mut self, undo_log: L2) -> SnapshotMap<K, V, &mut M, L2> {
        SnapshotMap { map: &mut self.map, undo_log, _marker: PhantomData }
    }
}

impl<K, V, M, L> SnapshotMap<K, V, M, L>
where
    K: Hash + Clone + Eq,
    M: AsMut<FxHashMap<K, V>> + AsRef<FxHashMap<K, V>>,
    L: UndoLogs<UndoLog<K, V>>,
{
    pub fn clear(&mut self) {
        self.map.as_mut().clear();
        self.undo_log.clear();
    }

    pub fn insert(&mut self, key: K, value: V) -> bool {
        match self.map.as_mut().insert(key.clone(), value) {
            None => {
                self.undo_log.push(UndoLog::Inserted(key));
                true
            }
            Some(old_value) => {
                self.undo_log.push(UndoLog::Overwrite(key, old_value));
                false
            }
        }
    }

    pub fn remove(&mut self, key: K) -> bool {
        match self.map.as_mut().remove(&key) {
            Some(old_value) => {
                self.undo_log.push(UndoLog::Overwrite(key, old_value));
                true
            }
            None => false,
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.map.as_ref().get(key)
    }
}

impl<K, V> SnapshotMap<K, V>
where
    K: Hash + Clone + Eq,
{
    pub fn snapshot(&mut self) -> Snapshot {
        self.undo_log.start_snapshot()
    }

    pub fn commit(&mut self, snapshot: Snapshot) {
        self.undo_log.commit(snapshot)
    }

    pub fn rollback_to(&mut self, snapshot: Snapshot) {
        self.undo_log.rollback_to(&mut self.map, snapshot)
    }
}

impl<'k, K, V, M, L> ops::Index<&'k K> for SnapshotMap<K, V, M, L>
where
    K: Hash + Clone + Eq,
    M: AsRef<FxHashMap<K, V>>,
{
    type Output = V;
    fn index(&self, key: &'k K) -> &V {
        &self.map.as_ref()[key]
    }
}

impl<K, V> Rollback<UndoLog<K, V>> for FxHashMap<K, V>
where
    K: Eq + Hash,
{
    fn reverse(&mut self, undo: UndoLog<K, V>) {
        match undo {
            UndoLog::Inserted(key) => {
                self.remove(&key);
            }

            UndoLog::Overwrite(key, old_value) => {
                self.insert(key, old_value);
            }

            UndoLog::Purged => {}
        }
    }
}
