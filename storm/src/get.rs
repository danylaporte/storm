use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
};

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

impl<K, V> Get<K, V> for HashMap<K, V>
where
    K: Eq + Hash,
{
    fn get(&self, k: &K) -> Option<&V> {
        HashMap::get(self, k)
    }
}

#[cfg(feature = "vec-map")]
impl<K, V> Get<K, V> for vec_map::VecMap<K, V>
where
    K: Clone + Into<usize>,
{
    fn get(&self, k: &K) -> Option<&V> {
        vec_map::VecMap::get(self, k)
    }
}
