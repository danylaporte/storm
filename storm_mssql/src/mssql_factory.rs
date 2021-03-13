use crate::MssqlProvider;
use async_trait::async_trait;
use storm::provider::ProviderFactory;
use tiberius::Config;

pub struct MssqlFactory(pub Config);

#[async_trait]
impl ProviderFactory for MssqlFactory {
    type Provider = MssqlProvider;

    async fn create_provider(&self) -> storm::Result<Self::Provider> {
        Ok(MssqlProvider::new(self.0.clone()))
    }
}
