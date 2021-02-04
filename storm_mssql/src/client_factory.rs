use crate::Client;
use async_trait::async_trait;
use storm::{Error, Result};
use tiberius::{Config, SqlBrowser};
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;
use tracing::instrument;

#[async_trait]
pub trait ClientFactory {
    async fn create_client(&self) -> Result<Client>;
}

#[async_trait]
impl ClientFactory for Config {
    #[instrument(name = "ClientFactory::create_client", skip(self), err)]
    async fn create_client(&self) -> Result<Client> {
        // named instance only available in windows.
        let tcp = TcpStream::connect_named(self).await.map_err(Error::std)?;
        tcp.set_nodelay(true).map_err(Error::std)?;

        let client = Client::connect(self.clone(), tcp.compat_write())
            .await
            .map_err(Error::std)?;

        Ok(client)
    }
}
