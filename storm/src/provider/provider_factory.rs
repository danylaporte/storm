use super::Provider;
use crate::Result;
use async_trait::async_trait;

#[async_trait]
pub trait ProviderFactory: Send + Sync + 'static {
    type Provider: Provider;

    async fn create_provider(&self) -> Result<Self::Provider>;
}
