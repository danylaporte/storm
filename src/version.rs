use crate::{EntitiesLoad, Entity, OptsVersion, Result, Table, TableAppyLog, TableLoad};
use async_trait::async_trait;
use std::ops::{Deref, DerefMut};

pub struct Version<T> {
    table: T,
    version: u64,
}

impl<T> Deref for Version<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.table
    }
}

impl<T> DerefMut for Version<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.table
    }
}

impl<T> Table for Version<T>
where
    T: Table,
{
    type Entity = T::Entity;
}

impl<T> TableAppyLog for Version<T>
where
    T: TableAppyLog,
{
    fn insert(&mut self, k: <Self::Entity as Entity>::Key, v: Self::Entity, version: u64) {
        self.version = version;
        T::insert(self, k, v, version);
    }

    fn remove(&mut self, k: &<Self::Entity as Entity>::Key, version: u64) {
        self.version = version;
        T::remove(self, k, version);
    }
}

#[async_trait]
impl<O, T> TableLoad<O> for Version<T>
where
    O: OptsVersion + Send + Sync,
    T: TableLoad<O>,
    T::Entity: EntitiesLoad<O>,
{
    async fn table_load(opts: &O) -> Result<Self> {
        Ok(Version {
            table: T::table_load(opts).await?,
            version: opts.opts_version(),
        })
    }
}
