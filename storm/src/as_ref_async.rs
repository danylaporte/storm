use crate::{BoxFuture, Ctx, Result};
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

#[allow(clippy::type_complexity)]
pub fn async_ref_block5<A, B, C, D, E>(
    ctx: &'_ Ctx,
) -> BoxFuture<'_, Result<(&'_ A, &'_ B, &'_ C, &'_ D, &'_ E)>>
where
    Ctx: AsRefAsync<A> + AsRefAsync<B> + AsRefAsync<C> + AsRefAsync<D> + AsRefAsync<E>,
    A: Sync,
    B: Sync,
    C: Sync,
    D: Sync,
    E: Sync,
{
    Box::pin(async move {
        Ok((
            crate::tri!(ctx.as_ref_async().await),
            crate::tri!(ctx.as_ref_async().await),
            crate::tri!(ctx.as_ref_async().await),
            crate::tri!(ctx.as_ref_async().await),
            crate::tri!(ctx.as_ref_async().await),
        ))
    })
}
