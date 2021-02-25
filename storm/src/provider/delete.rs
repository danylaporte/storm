use crate::{Entity, Result};
use async_trait::async_trait;

#[async_trait]
pub trait Delete<E: Entity> {
    async fn delete(&self, k: &E::Key) -> Result<()>;
}

#[async_trait]
impl<E> Delete<E> for ()
where
    E: Entity + 'static,
    E::Key: Sync,
{
    async fn delete(&self, _k: &E::Key) -> Result<()> {
        Ok(())
    }
}
