use crate::MssqlProvider;
use storm::{provider::ProviderFactory, BoxFuture, Result};
use tiberius::Config;

pub struct MssqlFactory(pub Config);

impl ProviderFactory for MssqlFactory {
    type Provider = MssqlProvider;

    fn create_provider<'a>(&'a self) -> BoxFuture<'a, Result<Self::Provider>> {
        Box::pin(async move { Ok(MssqlProvider::new(self.0.clone())) })
    }
}
