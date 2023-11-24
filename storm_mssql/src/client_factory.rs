use crate::{Client, Error};
use storm::{BoxFuture, Result};
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
    #[instrument(
        level = "debug",
        name = "ClientFactory::create_client",
        skip(self),
        err
    )]
    fn create_client(&self) -> BoxFuture<'_, Result<Client>> {
        Box::pin(async move {
            // named instance only available in windows.
            let tcp = TcpStream::connect_named(self)
                .await
                .map_err(Error::ConnectNamed)?;

            tcp.set_nodelay(true).map_err(Error::Io)?;

            Client::connect(self.clone(), tcp.compat_write())
                .await
                .map_err(Error::CreateClient)
                .map_err(Into::into)
        })
    }
}
