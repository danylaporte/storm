use super::Provider;
use crate::{BoxFuture, Result};

pub trait ProviderFactory: Send + Sync + 'static {
    type Provider: Provider;

    fn create_provider<'a>(&'a self) -> BoxFuture<'a, Result<Self::Provider>>;
}
