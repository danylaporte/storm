use crate::{BoxFuture, Entity, Result};

pub trait Upsert<E: Entity>: Send + Sync {
    fn upsert<'a>(&'a self, k: &'a E::Key, v: &'a E) -> BoxFuture<'a, Result<()>>;
}

impl<E, T> Upsert<E> for &T
where
    E: Entity + Sync,
    E::Key: Sync,
    T: Upsert<E> + Send + Sync,
{
    fn upsert<'a>(&'a self, k: &'a E::Key, v: &'a E) -> BoxFuture<'a, Result<()>> {
        (**self).upsert(k, v)
    }
}
