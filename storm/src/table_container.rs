use crate::{Entity, Result, Table, TableAppyLog, TableLoad, TableLog};
use async_cell_lock::AsyncOnceCell;
use async_trait::async_trait;

#[async_trait]
pub trait TableContainer<O> {
    type Table: Table;

    fn apply_log(&mut self, log: TableLog<Self::Table>, version: u64)
    where
        Self::Table: TableAppyLog;

    async fn ensure<'a>(&'a self, opts: &'a O) -> Result<&'a Self::Table>
    where
        Self::Table: TableLoad<O>;
}

#[async_trait]
impl<O, T> TableContainer<O> for AsyncOnceCell<T>
where
    O: Send + Sync,
    T: Table + Send + Sync,
    <T::Entity as Entity>::Key: PartialEq,
{
    type Table = T;

    fn apply_log(&mut self, log: TableLog<Self::Table>, version: u64)
    where
        Self::Table: TableAppyLog,
    {
        if let Some(table) = self.get_mut() {
            log.apply_log(table, version);
        }
    }

    async fn ensure<'a>(&'a self, opts: &'a O) -> Result<&'a Self::Table>
    where
        T: TableLoad<O>,
    {
        self.get_or_try_init(T::table_load(opts)).await
    }
}

#[cfg(feature = "cache")]
#[async_trait]
impl<K, O, S, V> TableContainer<O> for cache::Cache<K, V, S>
where
    K: PartialEq + Send + Sync,
    O: Send + Sync,
    S: Send + Sync,
    Self: Table<Entity = V>,
    V: Entity<Key = K> + Send + Sync,
{
    type Table = Self;

    fn apply_log(&mut self, log: TableLog<Self::Table>, version: u64)
    where
        Self::Table: TableAppyLog,
    {
        log.apply_log(self, version);
    }

    async fn ensure<'a>(&'a self, _opts: &'a O) -> Result<&'a Self::Table>
    where
        Self: TableLoad<O>,
    {
        Ok(self)
    }
}
