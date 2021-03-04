use crate::{Entity, Result};
use async_trait::async_trait;

#[async_trait]
pub trait LoadAll<E: Entity, FILTER: Send + Sync> {
    async fn load_all<C: Default + Extend<(E::Key, E)> + Send>(&self, filter: &FILTER)
        -> Result<C>;
}

#[async_trait]
impl<E: Entity + 'static, P> LoadAll<E, ()> for &P
where
    P: LoadAll<E, ()> + Send + Sync,
{
    async fn load_all<C: Default + Extend<(E::Key, E)> + Send>(&self, filter: &()) -> Result<C> {
        (**self).load_all(filter).await
    }
}

#[async_trait]
impl<E: Entity + 'static> LoadAll<E, ()> for () {
    async fn load_all<C: Default + Extend<(E::Key, E)>>(&self, _: &()) -> Result<C> {
        Ok(C::default())
    }
}
