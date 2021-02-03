use crate::{Entity, Result};
use async_trait::async_trait;

#[async_trait]
pub trait EntitiesLoad<O>: Entity + Sized {
    async fn entities_load(opts: &O) -> Result<Vec<(Self::Key, Self)>>;
}
