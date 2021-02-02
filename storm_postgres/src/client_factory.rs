use async_trait::async_trait;
use native_tls::TlsConnector;
use postgres_native_tls::MakeTlsConnector;
use storm::{Error, Result};
use tokio::spawn;
use tokio_postgres::{Client, Config};
use tracing::{error, instrument};

#[async_trait]
pub trait ClientFactory {
    async fn create_client(&self) -> Result<Client>;
}

#[async_trait]
impl ClientFactory for Config {
    #[instrument(name = "ClientFactory::create_client", skip(self), err)]
    async fn create_client(&self) -> Result<Client> {
        let connector = TlsConnector::builder().build().map_err(Error::std)?;
        let connector = MakeTlsConnector::new(connector);

        let (client, connection) = self.connect(connector).await.map_err(Error::std)?;

        spawn(async move {
            if let Err(e) = connection.await {
                error!("connection error: {}", e);
            }
        });

        Ok(client)
    }
}
