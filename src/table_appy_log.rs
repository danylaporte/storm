use crate::{Entity, Table};
use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
};

pub trait TableAppyLog: Table {
    fn insert(&mut self, k: <Self::Entity as Entity>::Key, v: Self::Entity);

    fn remove(&mut self, k: &<Self::Entity as Entity>::Key);
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

#[cfg(feature = "vec-map")]
impl<K, V> TableAppyLog for vec_map::VecMap<K, V>
where
    K: Clone + Into<usize>,
    V: Entity<Key = K>,
{
    fn insert(&mut self, k: K, v: V) {
        vec_map::VecMap::insert(self, k, v);
    }

    fn remove(&mut self, k: &K) {
        vec_map::VecMap::remove(self, k);
    }
}
