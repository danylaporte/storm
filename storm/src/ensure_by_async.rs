use crate::{provider::LoadOne, Entity, ProviderContainer, Result};
use async_trait::async_trait;
use cache::CacheIsland;

#[async_trait]
pub trait EnsureByAsync<E: Entity> {
    async fn ensure_by_async(&mut self, key: &E::Key, provider: &ProviderContainer) -> Result<()>;
}

#[cfg(feature = "cache")]
#[async_trait]
impl<E> EnsureByAsync<E> for CacheIsland<E>
where
    E: Entity,
    ProviderContainer: LoadOne<E>,
{
    async fn ensure_by_async(&mut self, key: &E::Key, provider: &ProviderContainer) -> Result<()> {
        if self.get().is_none() {
            if let Some(v) = provider.load_one(key).await? {
                self.set(v);
            }
        }

        Ok(())
    }
}

#[async_trait]
impl<E> EnsureByAsync<E> for Option<E>
where
    E: Entity,
    ProviderContainer: LoadOne<E>,
{
    async fn ensure_by_async(&mut self, key: &E::Key, provider: &ProviderContainer) -> Result<()> {
        if self.is_none() {
            if let Some(v) = provider.load_one(key).await? {
                *self = Some(v);
            }
        }

        Ok(())
    }
}
