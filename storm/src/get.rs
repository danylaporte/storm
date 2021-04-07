use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
};

use vec_map::VecMap;

pub trait Get<K, V> {
    fn get(&self, k: &K) -> Option<&V>;
}

impl<K, V, T> Get<K, V> for &T
where
    T: Get<K, V>,
{
    fn get(&self, k: &K) -> Option<&V> {
        (*self).get(k)
    }
}

impl<K, V, T> Get<K, V> for &mut T
where
    T: Get<K, V>,
{
    fn get(&self, k: &K) -> Option<&V> {
        (**self).get(k)
    }
}

#[cfg(feature = "cache")]
impl<K, V, S> Get<K, V> for cache::Cache<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    fn get(&self, k: &K) -> Option<&V> {
        cache::Cache::get(self, k)
    }
}

impl<K, V, S> Get<K, V> for HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher,
{
    fn get(&self, k: &K) -> Option<&V> {
        HashMap::get(self, k)
    }
}

impl<K, V> Get<K, V> for VecMap<K, V>
where
    K: Clone + Into<usize>,
{
    fn get(&self, k: &K) -> Option<&V> {
        VecMap::get(self, k)
    }
}
