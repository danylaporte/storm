use crate::{Error, MssqlProvider};
use std::{env::var, ffi::OsStr};
use storm::{provider::ProviderFactory, BoxFuture, Result};
use tiberius::Config;

pub struct MssqlFactory(pub Config);

impl MssqlFactory {
    pub fn from_env<K>(var_name: K) -> Result<Self>
    where
        K: AsRef<OsStr>,
    {
        let name = var_name.as_ref();

        let conn_str = var(name).map_err(|source| Error::Var {
            name: name.to_string_lossy().to_string(),
            source,
        })?;

        let config = Config::from_ado_string(&conn_str).map_err(Error::ParseAdoConnStr)?;

        Ok(Self(config))
    }
}

impl ProviderFactory for MssqlFactory {
    type Provider = MssqlProvider;

    fn create_provider(&self) -> BoxFuture<'_, Result<Self::Provider>> {
        Box::pin(async move { Ok(MssqlProvider::new(self.0.clone())) })
    }
}
