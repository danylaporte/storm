use crate::MssqlProvider;
use std::{env::var, ffi::OsStr};
use storm::{provider::ProviderFactory, BoxFuture, Error, Result};
use tiberius::Config;

pub struct MssqlFactory(pub Config);

impl MssqlFactory {
    pub fn from_env<K>(var_name: K) -> Result<Self>
    where
        K: AsRef<OsStr>,
    {
        Ok(Self(Config::from_ado_string(
            &var(var_name).map_err(Error::std)?,
        )?))
    }
}

impl ProviderFactory for MssqlFactory {
    type Provider = MssqlProvider;

    fn create_provider(&self) -> BoxFuture<'_, Result<Self::Provider>> {
        Box::pin(async move { Ok(MssqlProvider::new(self.0.clone())) })
    }
}
