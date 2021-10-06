use crate::Entity;

pub trait GetMut<E: Entity> {
    fn get_mut(&mut self, k: &E::Key) -> Option<&mut E>;
}

#[cfg(feature = "cache")]
impl<E, S> GetMut<E> for cache::Cache<E::Key, E, S>
where
    E: Entity,
    E::Key: Eq + std::hash::Hash,
    S: std::hash::BuildHasher,
{
    fn get_mut(&mut self, k: &E::Key) -> Option<&mut E> {
        cache::Cache::get_mut(self, k)
    }
}
