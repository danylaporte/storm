use crate::{Entity, Result};
use async_trait::async_trait;

#[async_trait]
pub trait Upsert<E: Entity> {
    async fn upsert(&self, k: &E::Key, v: &E) -> Result<()>;
}

#[async_trait]
impl<E, T> Upsert<E> for &T
where
    E: Entity + Sync,
    E::Key: Sync,
    T: Upsert<E> + Send + Sync,
{
    async fn upsert(&self, k: &E::Key, v: &E) -> Result<()> {
        (**self).upsert(k, v).await
    }
}
