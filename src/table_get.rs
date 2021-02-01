use crate::{Entity, Table};
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
