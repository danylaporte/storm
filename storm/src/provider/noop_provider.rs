use super::{Provider, ProviderFactory};
use crate::{BoxFuture, Result};

/// A provider that loads nothing and persists nothing; the context tables are the only storage.
///
/// Register it in place of a database provider (under the same name) to run without a database.
pub struct NoopProvider;

impl Provider for NoopProvider {
    fn cancel(&self) {}

    fn commit(&self) -> BoxFuture<'_, Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

pub struct NoopFactory;

impl ProviderFactory for NoopFactory {
    type Provider = NoopProvider;

    fn create_provider(&self) -> BoxFuture<'_, Result<Self::Provider>> {
        Box::pin(async { Ok(NoopProvider) })
    }
}
