use crate::{Client, ClientFactory, Error, MssqlFactory, MssqlProvider};
use storm::{provider::ProviderFactory, BoxFuture, Result};

/// This can wrap a ClientFactory and creates a transaction for each Client that are returned.
/// It is useful for integration tests making sure that all items are rollback once the test
/// is done.
pub struct TransactionScoped<F>(pub(crate) F);

impl From<MssqlFactory> for TransactionScoped<MssqlFactory> {
    fn from(f: MssqlFactory) -> Self {
        TransactionScoped(f)
    }
}

impl ProviderFactory for TransactionScoped<MssqlFactory> {
    type Provider = MssqlProvider;

    fn create_provider(&self) -> BoxFuture<'_, Result<Self::Provider>> {
        Box::pin(async move { Ok(MssqlProvider::new(TransactionScoped(self.0 .0.clone()))) })
    }
}

impl<F> ClientFactory for TransactionScoped<F>
where
    F: ClientFactory + Send + Sync,
{
    fn create_client(&self) -> BoxFuture<'_, Result<Client>> {
        Box::pin(async {
            let mut client = self.0.create_client().await?;

            client
                .simple_query("BEGIN TRAN")
                .await
                .map_err(Error::unknown)?;

            Ok(client)
        })
    }

    fn under_transaction(&self) -> bool {
        true
    }
}
