use crate::Entity;
use std::hash::{BuildHasher, Hash};

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
