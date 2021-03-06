use super::{LoadAll, ProviderContainer};
use crate::{Entity, Result};
use async_trait::async_trait;
use std::ops::Deref;

pub struct TransactionProvider<'a>(pub(super) &'a ProviderContainer);

impl<'a> TransactionProvider<'a> {
    pub async fn commit(&self) -> Result<()> {
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

#[async_trait]
impl<'a, C, E, FILTER> LoadAll<E, FILTER, C> for TransactionProvider<'a>
where
    C: Default + Extend<(E::Key, E)> + Send + 'static,
    E: Entity + 'static,
    FILTER: Send + Sync,
    ProviderContainer: LoadAll<E, FILTER, C>,
{
    async fn load_all(&self, filter: &FILTER) -> Result<C> {
        self.0.load_all(filter).await
    }
}
