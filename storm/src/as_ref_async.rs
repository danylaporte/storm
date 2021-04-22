use async_cell_lock::QueueRwLockReadGuard;

use crate::{BoxFuture, Result};

pub trait AsRefAsync<T> {
    fn as_ref_async(&self) -> BoxFuture<'_, Result<&'_ T>>;
}

impl<T, U> AsRefAsync<T> for QueueRwLockReadGuard<'_, U>
where
    U: AsRefAsync<T>,
{
    fn as_ref_async(&self) -> BoxFuture<'_, Result<&'_ T>> {
        (&**self).as_ref_async()
    }
}
