use crate::{Entity, Result};
use async_trait::async_trait;

#[async_trait]
pub trait Upsert<E: Entity> {
    async fn upsert(&self, k: &E::Key, v: &E) -> Result<()>;
}
