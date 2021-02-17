use crate::{Entity, Result};
use async_trait::async_trait;

#[async_trait]
pub trait LoadAll<E: Entity, FILTER: Send + Sync> {
    async fn load_all<C: Default + Extend<(E::Key, E)> + Send>(&self, filter: &FILTER)
        -> Result<C>;
}
