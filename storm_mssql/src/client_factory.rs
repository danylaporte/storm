use crate::Client;
use storm::{BoxFuture, Error, Result};
use tiberius::{Config, SqlBrowser};
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;
use tracing::instrument;

pub trait ClientFactory: Send + Sync + 'static {
    fn create_client(&self) -> BoxFuture<'_, Result<Client>>;

    /// Indicate if the client factory operate under a transaction. This is useful for
    /// tests operations where all commits are rollback after the tests.
    fn under_transaction(&self) -> bool {
        false
    }
}

impl ClientFactory for Config {
    fn create_client(&self) -> BoxFuture<'_, Result<Client>> {
        Box::pin(config_create_client(self))
    }
}

#[instrument(name = "ClientFactory::create_client", skip(config), err)]
async fn config_create_client(config: &Config) -> Result<Client> {
    // named instance only available in windows.
    let tcp = TcpStream::connect_named(config).await.map_err(Error::std)?;
    tcp.set_nodelay(true).map_err(Error::std)?;

    Client::connect(config.clone(), tcp.compat_write())
        .await
        .map_err(Into::into)
}
