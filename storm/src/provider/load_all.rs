use crate::{Entity, Result};
use async_trait::async_trait;

#[async_trait]
pub trait LoadAll<E: Entity, FILTER: Send + Sync, C>
where
    C: Default + Extend<(E::Key, E)> + Send,
{
    async fn load_all(&self, filter: &FILTER) -> Result<C>;
}

#[async_trait]
impl<C, E, FILTER, P> LoadAll<E, FILTER, C> for &P
where
    C: Default + Extend<(E::Key, E)> + Send + 'static,
    E: Entity + 'static,
    FILTER: Send + Sync,
    P: LoadAll<E, FILTER, C> + Send + Sync,
{
    async fn load_all(&self, filter: &FILTER) -> Result<C> {
        (**self).load_all(filter).await
    }
}
