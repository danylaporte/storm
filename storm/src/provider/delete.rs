use crate::{Entity, Result};
use async_trait::async_trait;

#[async_trait]
pub trait Delete<E: Entity> {
    async fn delete(&self, k: &E::Key) -> Result<()>;
}
