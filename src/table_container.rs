use crate::{ApplyLog, LoadTable, Result, Table, TableLog};
use async_cell_lock::AsyncOnceCell;
use async_trait::async_trait;

#[async_trait]
pub trait TableContainer<O> {
    type Table: Table;

    fn apply_log(&mut self, log: TableLog<<Self::Table as ApplyLog>::Row>)
    where
        Self::Table: ApplyLog;

    async fn ensure<'a>(&'a self, opts: &O) -> Result<&'a Self::Table>
    where
        Self::Table: LoadTable<O>;
}

#[async_trait]
impl<O, T> TableContainer<O> for AsyncOnceCell<T>
where
    O: Send + Sync,
    T: Table + Send + Sync,
{
    type Table = T;

    fn apply_log(&mut self, log: TableLog<<T as ApplyLog>::Row>)
    where
        T: ApplyLog,
    {
        if let Some(table) = self.get_mut() {
            table.apply_log(log);
        }
    }

    async fn ensure<'a>(&'a self, opts: &O) -> Result<&'a Self::Table>
    where
        T: LoadTable<O>,
    {
        self.get_or_try_init(T::load_table(opts)).await
    }
}
