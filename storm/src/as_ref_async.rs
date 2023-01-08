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
        let a = ctx.as_ref_async().await;
        let b = ctx.as_ref_async().await;
        let c = ctx.as_ref_async().await;
        let d = ctx.as_ref_async().await;
        let e = ctx.as_ref_async().await;

        a.and_then(|a| b.and_then(|b| c.and_then(|c| d.and_then(|d| e.map(|e| (a, b, c, d, e))))))
    })
}
