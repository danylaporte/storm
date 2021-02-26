use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
};

use crate::Entity;

pub trait Get<E: Entity> {
    fn get(&self, k: &E::Key) -> Option<&E>;
}

impl<E, T> Get<E> for &T
where
    E: Entity,
    T: Get<E>,
{
    fn get(&self, k: &E::Key) -> Option<&E> {
        (*self).get(k)
    }
}

impl<E, T> Get<E> for &mut T
where
    E: Entity,
    T: Get<E>,
{
    fn get(&self, k: &E::Key) -> Option<&E> {
        (**self).get(k)
    }
}

#[cfg(feature = "cache")]
impl<E, S> Get<E> for cache::Cache<E::Key, E, S>
where
    E: Entity,
    E::Key: Eq + Hash,
    S: BuildHasher,
{
    fn get(&self, k: &E::Key) -> Option<&E> {
        cache::Cache::get(self, k)
    }
}

impl<E, S> Get<E> for HashMap<E::Key, E, S>
where
    E: Entity,
    E::Key: Eq + Hash,
    S: BuildHasher,
{
    fn get(&self, k: &E::Key) -> Option<&E> {
        HashMap::get(self, k)
    }
}

#[cfg(feature = "vec-map")]
impl<E> Get<E> for vec_map::VecMap<E::Key, E>
where
    E: Entity,
    E::Key: Clone + Into<usize>,
{
    fn get(&self, k: &E::Key) -> Option<&E> {
        vec_map::VecMap::get(self, k)
    }
}
