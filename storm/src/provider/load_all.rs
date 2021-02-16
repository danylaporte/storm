use crate::{Entity, Result};
use async_trait::async_trait;

#[async_trait]
pub trait LoadAll<E: Entity> {
    async fn load_all<C: Default + Extend<(E::Key, E)> + Send>(&self) -> Result<C>;
}
