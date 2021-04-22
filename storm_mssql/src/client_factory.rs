use crate::Client;
use storm::{BoxFuture, Error, Result};
use tiberius::{Config, SqlBrowser};
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;
use tracing::instrument;

pub trait ClientFactory: Send + Sync + 'static {
    fn create_client<'a>(&'a self) -> BoxFuture<'a, Result<Client>>;
}

impl ClientFactory for Config {
    #[instrument(name = "ClientFactory::create_client", skip(self), err)]
    fn create_client<'a>(&'a self) -> BoxFuture<'a, Result<Client>> {
        Box::pin(async move {
            // named instance only available in windows.
            let tcp = TcpStream::connect_named(self).await.map_err(Error::std)?;
            tcp.set_nodelay(true).map_err(Error::std)?;

            let client = Client::connect(self.clone(), tcp.compat_write())
                .await
                .map_err(Error::std)?;

            Ok(client)
        })
    }
}
