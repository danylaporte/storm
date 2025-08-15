use crate::{logs::TableLog, Entity, LogOf};

pub trait Table<E: Entity>:
    Default + Extend<(E::Key, E)> + LogOf<Log = TableLog<E>> + Send + Sync
{
    fn get(&self, key: &E::Key) -> Option<&E>;

    #[inline]
    fn contains(&self, key: &E::Key) -> bool {
        self.get(key).is_some()
    }
}
