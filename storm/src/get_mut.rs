use crate::Entity;
use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
};

pub trait GetMut<E: Entity> {
    fn get_mut(&mut self, k: &E::Key) -> Option<&mut E>;
}

#[cfg(feature = "cache")]
impl<E, S> GetMut<E> for cache::Cache<E::Key, E, S>
where
    E: Entity,
    E::Key: Eq + Hash,
    S: BuildHasher,
{
    fn get_mut(&mut self, k: &E::Key) -> Option<&mut E> {
        cache::Cache::get_mut(self, k)
    }
}

impl<E, S> GetMut<E> for HashMap<E::Key, E, S>
where
    E: Entity,
    E::Key: Eq + Hash,
    S: BuildHasher,
{
    fn get_mut(&mut self, k: &E::Key) -> Option<&mut E> {
        HashMap::get_mut(self, k)
    }
}

#[cfg(feature = "vec-map")]
impl<E> GetMut<E> for vec_map::VecMap<E::Key, E>
where
    E: Entity,
    E::Key: Clone + Into<usize>,
{
    fn get_mut(&mut self, k: &E::Key) -> Option<&mut E> {
        vec_map::VecMap::get_mut(self, k)
    }
}
