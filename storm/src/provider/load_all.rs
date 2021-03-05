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
impl<C, E: Entity + 'static, P> LoadAll<E, (), C> for &P
where
    C: Default + Extend<(E::Key, E)> + Send + 'static,
    P: LoadAll<E, (), C> + Send + Sync,
{
    async fn load_all(&self, filter: &()) -> Result<C> {
        (**self).load_all(filter).await
    }
}

#[async_trait]
impl<C, E: Entity + 'static> LoadAll<E, (), C> for ()
where
    C: Default + Extend<(E::Key, E)> + Send + 'static,
{
    async fn load_all(&self, _: &()) -> Result<C> {
        Ok(C::default())
    }
}
