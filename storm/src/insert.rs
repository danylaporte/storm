use crate::{Entity, Result};
use async_trait::async_trait;

#[async_trait]
pub trait Insert<E: Entity> {
    async fn insert(&mut self, k: E::Key, v: E) -> Result<()>;
}
