use crate::{
    async_cell_lock::QueueRwLockQueueGuard, indexing::IndexLogs, ApplyLog, Ctx, Entity,
    EntityAccessor, Get, LogState, LogsVar, Result,
};
use fxhash::FxHashMap;
use std::{collections::hash_map::Entry, hash::Hash, mem::replace};

pub struct Logs(pub(crate) LogsVar);

impl Logs {
    pub async fn apply_log(self, ctx: QueueRwLockQueueGuard<'_, Ctx>) -> Result<bool> {
        Ok(ctx.write().await?.apply_log(self))
    }
}

pub struct Log<E: Entity> {
    pub changes: FxHashMap<E::Key, LogState<E>>,
    pub(crate) indexes: IndexLogs,
}

impl<E> Log<E>
where
    E: EntityAccessor,
    E::Tbl: Get<E>,
    E::Key: Clone + Eq + Hash,
{
    pub fn insert(&mut self, tbl: &E::Tbl, key: E::Key, value: E) {
        match self.changes.entry(key) {
            Entry::Occupied(mut o) => {
                let old = match o.get() {
                    LogState::Inserted(o) => Some(o),
                    LogState::Removed => None,
                };

                tbl.as_ref().upsert(&mut self.indexes, o.key(), &value, old);
                o.insert(LogState::Inserted(value));
            }
            Entry::Vacant(v) => {
                let old = tbl.get(v.key());
                tbl.as_ref().upsert(&mut self.indexes, v.key(), &value, old);
                v.insert(LogState::Inserted(value));
            }
        }
    }

    /// Returns true if the value is present
    pub fn remove(&mut self, tbl: &E::Tbl, key: &E::Key) -> bool {
        match self.changes.entry(key.clone()) {
            Entry::Occupied(mut o) => match replace(o.get_mut(), LogState::Removed) {
                LogState::Inserted(v) => {
                    tbl.as_ref().remove(&mut self.indexes, key, &v);
                    true
                }
                LogState::Removed => false,
            },
            Entry::Vacant(v) => match tbl.get(key) {
                Some(e) => {
                    tbl.as_ref().remove(&mut self.indexes, key, e);
                    v.insert(LogState::Removed);
                    true
                }
                None => false,
            },
        }
    }
}

impl<E: Entity> Default for Log<E> {
    fn default() -> Self {
        Self {
            changes: Default::default(),
            indexes: Default::default(),
        }
    }
}
