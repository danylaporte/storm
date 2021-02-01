use crate::{Entity, LoadTable, Result, Table, TableAppyLog, TableLog};
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
        Self::Table: LoadTable<O>;
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
        T: LoadTable<O>,
    {
        self.get_or_try_init(T::load_table(opts)).await
    }
}
