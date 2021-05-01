use super::{LoadAll, ProviderContainer};
use crate::{BoxFuture, Entity, Result};
use std::ops::Deref;

pub struct TransactionProvider<'a>(pub(super) &'a ProviderContainer);

impl<'a> TransactionProvider<'a> {
    pub fn commit(&self) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            let mut error = None;

            for provider in self.0.iter_transaction() {
                if error.is_none() {
                    if let Err(e) = provider.commit().await {
                        error = Some(e);
                    }
                } else {
                    provider.cancel();
                }
            }

            match error {
                Some(err) => Err(err),
                None => Ok(()),
            }
        })
    }

    pub fn container(&self) -> &'a ProviderContainer {
        self.0
    }
}

impl<'a> Deref for TransactionProvider<'a> {
    type Target = ProviderContainer;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a> Drop for TransactionProvider<'a> {
    fn drop(&mut self) {
        for provider in self.0.iter_transaction() {
            provider.cancel();
        }
    }
}

impl<C, E, FILTER> LoadAll<E, FILTER, C> for TransactionProvider<'_>
where
    C: Default + Extend<(E::Key, E)> + Send + 'static,
    E: Entity + 'static,
    FILTER: Send + Sync,
    ProviderContainer: LoadAll<E, FILTER, C>,
{
    fn load_all<'a>(&'a self, filter: &'a FILTER) -> BoxFuture<'a, Result<C>> {
        self.0.load_all(filter)
    }
}
