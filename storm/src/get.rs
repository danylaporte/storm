use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
};

use vec_map::VecMap;

use crate::Entity;

pub trait Get<E: Entity> {
    fn get(&self, k: &E::Key) -> Option<&E>;
}

impl<E: Entity, T> Get<E> for &T
where
    T: Get<E>,
{
    fn get(&self, k: &E::Key) -> Option<&E> {
        (*self).get(k)
    }
}

impl<E: Entity, T> Get<E> for &mut T
where
    T: Get<E>,
{
    fn get(&self, k: &E::Key) -> Option<&E> {
        (**self).get(k)
    }
}

#[cfg(feature = "cache")]
impl<E: Entity, S> Get<E> for cache::Cache<E::Key, E, S>
where
    E::Key: Eq + Hash,
    S: BuildHasher,
{
    fn get(&self, k: &E::Key) -> Option<&E> {
        cache::Cache::get(self, k)
    }
}

impl<E: Entity, S> Get<E> for HashMap<E::Key, E, S>
where
    E::Key: Eq + Hash,
    S: BuildHasher,
{
    fn get(&self, k: &E::Key) -> Option<&E> {
        HashMap::get(self, k)
    }
}

impl<E: Entity> Get<E> for VecMap<E::Key, E>
where
    E::Key: Copy + Into<usize>,
{
    fn get(&self, k: &E::Key) -> Option<&E> {
        VecMap::get(self, k)
    }
}
