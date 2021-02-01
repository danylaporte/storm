use crate::{Entity, Table};
use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
};

pub trait TableAppyLog: Table {
    fn insert(&mut self, k: <Self::Entity as Entity>::Key, v: Self::Entity, version: u64);

    fn remove(&mut self, k: &<Self::Entity as Entity>::Key, version: u64);
}

impl<K, V, S> TableAppyLog for HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
    V: Entity<Key = K>,
{
    fn insert(&mut self, k: K, v: V, _: u64) {
        HashMap::insert(self, k, v);
    }

    fn remove(&mut self, k: &K, _: u64) {
        HashMap::remove(self, k);
    }
}

#[cfg(feature = "vec-map")]
impl<K, V> TableAppyLog for vec_map::VecMap<K, V>
where
    K: Clone + Into<usize>,
    V: Entity<Key = K>,
{
    fn insert(&mut self, k: K, v: V, _: u64) {
        vec_map::VecMap::insert(self, k, v);
    }

    fn remove(&mut self, k: &K, _: u64) {
        vec_map::VecMap::remove(self, k);
    }
}
