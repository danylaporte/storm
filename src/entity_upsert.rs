use crate::{Entity, Result};
use async_trait::async_trait;

#[async_trait]
pub trait EntityUpsert<O>: Entity {
    async fn entity_upsert(&self, key: &Self::Key, opts: &O) -> Result<()>;
}
