use crate::Entity;
use std::hash::{BuildHasher, Hash};

pub trait Get<E, Q: ?Sized> {
    fn get_entity<'a>(&'a self, q: &Q) -> Option<&'a E>;
}

#[cfg(feature = "cache")]
impl<E, Q, S> Get<E, Q> for cache::Cache<E::Key, E, S>
where
    E: Entity<Key = Q>,
    Q: Eq + Hash,
    S: BuildHasher,
{
    fn get_entity<'a>(&'a self, q: &Q) -> Option<&'a E> {
        cache::Cache::get(self, q)
    }
}

pub trait GetOwned<'a, E, Q: ?Sized> {
    fn get_owned(self, q: &Q) -> Option<&'a E>;
}
