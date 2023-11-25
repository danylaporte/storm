use crate::{Error, MssqlProvider};
use std::env::var;
use storm::{provider::ProviderFactory, BoxFuture, Result};
use tiberius::Config;

pub struct MssqlFactory(pub Config);

impl MssqlFactory {
    pub fn from_env_with_trust(var_name: &str, trust: bool) -> Result<Self> {
        let conn_str = var(var_name).map_err(|error| Error::Var {
            error,
            name: var_name.to_string(),
        })?;

        let mut config = Config::from_ado_string(&conn_str).map_err(Error::ParseAdoConnStr)?;

        if trust {
            config.trust_cert();
        }

        Ok(Self(config))
    }
}

impl ProviderFactory for MssqlFactory {
    type Provider = MssqlProvider;

    fn create_provider(&self) -> BoxFuture<'_, Result<Self::Provider>> {
        Box::pin(async move { Ok(MssqlProvider::new(self.0.clone())) })
    }
}
