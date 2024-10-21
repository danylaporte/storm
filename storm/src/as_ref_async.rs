use crate::{BoxFuture, Result};
use async_cell_lock::QueueRwLockReadGuard;

pub trait AsRefAsync<T> {
    fn as_ref_async(&self) -> BoxFuture<'_, Result<&'_ T>>;
}

impl<T, U> AsRefAsync<T> for QueueRwLockReadGuard<'_, U>
where
    U: AsRefAsync<T>,
{
    fn as_ref_async(&self) -> BoxFuture<'_, Result<&'_ T>> {
        (**self).as_ref_async()
    }
}
