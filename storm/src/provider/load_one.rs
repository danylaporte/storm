use crate::{Entity, Result};
use async_trait::async_trait;

#[async_trait]
pub trait LoadOne<E: Entity> {
    async fn load_one(&self, k: &E::Key) -> Result<Option<E>>;
}
