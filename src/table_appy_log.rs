use crate::{Entity, Table};
use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
};
use vec_map::VecMap;

pub trait TableAppyLog: Table {
    fn insert(&mut self, k: <Self::Entity as Entity>::Key, v: Self::Entity);

    fn remove(&mut self, k: &<Self::Entity as Entity>::Key);
}

impl<T> TableAppyLog for &mut T
where
    T: TableAppyLog,
{
    fn insert(&mut self, k: <Self::Entity as Entity>::Key, v: Self::Entity) {
        (**self).insert(k, v);
    }

    fn remove(&mut self, k: &<Self::Entity as Entity>::Key) {
        (**self).remove(k);
    }
}

impl<K, V, S> TableAppyLog for HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
    V: Entity<Key = K>,
{
    fn insert(&mut self, k: K, v: V) {
        HashMap::insert(self, k, v);
    }

    fn remove(&mut self, k: &K) {
        HashMap::remove(self, k);
    }
}

impl<K, V> TableAppyLog for VecMap<K, V>
where
    K: Clone + Into<usize>,
    V: Entity<Key = K>,
{
    fn insert(&mut self, k: K, v: V) {
        VecMap::insert(self, k, v);
    }

    fn remove(&mut self, k: &K) {
        VecMap::remove(self, k);
    }
}
