use crate::{Entity, Result};
use async_trait::async_trait;

#[async_trait]
pub trait Delete<E: Entity> {
    async fn delete(&self, k: &E::Key) -> Result<()>;
}

#[async_trait]
impl<'a, E, T> Delete<E> for &'a T
where
    E: Entity + 'a,
    E::Key: Sync,
    T: Delete<E> + Send + Sync,
{
    async fn delete(&self, k: &E::Key) -> Result<()> {
        (**self).delete(k).await
    }
}
