use crate::Entity;
use std::hash::{BuildHasher, Hash};

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
