use crate::{Entity, Table, TableLog, TableTransaction};
use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
};

pub trait TableGet: Table {
    fn get(&self, k: &<Self::Entity as Entity>::Key) -> Option<&Self::Entity>;

    fn contains_key(&self, k: &<Self::Entity as Entity>::Key) -> bool {
        self.get(k).is_some()
    }
}

impl<K, V, S> TableGet for HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
    V: Entity<Key = K>,
{
    fn get(&self, k: &K) -> Option<&V> {
        HashMap::<K, V, S>::get(self, k)
    }
}

impl<'a, L, O, T> TableGet for TableTransaction<'a, L, O, T>
where
    <T::Entity as Entity>::Key: PartialEq,
    L: AsRef<TableLog<T>>,
    T: TableGet,
{
    fn get(&self, k: &<Self::Entity as Entity>::Key) -> Option<&Self::Entity> {
        TableTransaction::get(self, k)
    }
}

#[cfg(feature = "vec-map")]
impl<K, V> TableGet for vec_map::VecMap<K, V>
where
    K: Clone + Into<usize>,
    V: Entity<Key = K>,
{
    fn get(&self, k: &K) -> Option<&V> {
        vec_map::VecMap::<K, V>::get(self, k)
    }
}
