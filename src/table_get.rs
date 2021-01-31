use crate::{Entity, Table, TableTransaction};
use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
};
use vec_map::VecMap;

pub trait TableGet: Table {
    fn get(&self, k: &<Self::Entity as Entity>::Key) -> Option<&Self::Entity>;

    fn contains_key(&self, k: &<Self::Entity as Entity>::Key) -> bool {
        self.get(k).is_some()
    }
}

impl<T> TableGet for &T
where
    T: TableGet,
{
    fn get(&self, k: &<Self::Entity as Entity>::Key) -> Option<&Self::Entity> {
        (**self).get(k)
    }
}

impl<T> TableGet for &mut T
where
    T: TableGet,
{
    fn get(&self, k: &<Self::Entity as Entity>::Key) -> Option<&Self::Entity> {
        (**self).get(k)
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

impl<'a, T: TableGet> TableGet for TableTransaction<'a, T>
where
    <T::Entity as Entity>::Key: PartialEq,
{
    fn get(&self, k: &<Self::Entity as Entity>::Key) -> Option<&Self::Entity> {
        TableTransaction::get(self, k)
    }
}

impl<K, V> TableGet for VecMap<K, V>
where
    K: Clone + Into<usize>,
    V: Entity<Key = K>,
{
    fn get(&self, k: &K) -> Option<&V> {
        VecMap::<K, V>::get(self, k)
    }
}
