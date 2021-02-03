use crate::{Entity, Result};
use async_trait::async_trait;

#[async_trait]
pub trait EntityDelete<O>: Entity {
    async fn entity_delete(key: &Self::Key, opts: &O) -> Result<()>;
}
