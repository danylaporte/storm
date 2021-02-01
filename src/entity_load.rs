use crate::{Entity, Result};
use async_trait::async_trait;

#[async_trait]
pub trait EntityLoad<O>: Entity + Sized {
    async fn entity_load(opts: &O) -> Result<Vec<(Self::Key, Self)>>;
}
