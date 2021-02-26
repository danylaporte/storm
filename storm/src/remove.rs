use crate::{Entity, Result};
use async_trait::async_trait;

#[async_trait]
pub trait Remove<E: Entity> {
    async fn remove(&mut self, k: E::Key) -> Result<()>;
}
