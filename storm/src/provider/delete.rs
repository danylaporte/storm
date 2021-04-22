use crate::{BoxFuture, Entity, Result};

pub trait Delete<E: Entity>: Send + Sync {
    fn delete<'a>(&'a self, k: &'a E::Key) -> BoxFuture<'a, Result<()>>;
}

impl<E, T> Delete<E> for &T
where
    E: Entity,
    E::Key: Sync,
    T: Delete<E>,
{
    fn delete<'a>(&'a self, k: &'a E::Key) -> BoxFuture<'a, Result<()>> {
        (**self).delete(k)
    }
}
