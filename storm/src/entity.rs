use crate::{BoxFuture, CtxTransaction, Result};
use std::{fmt::Debug, hash::Hash};

pub trait Entity: Send + Sync + 'static {
    type Key: Clone + Debug + Eq + Hash + Send + Sync;

    #[allow(unused_variables)]
    fn track_insert<'a>(
        &'a self,
        key: &'a Self::Key,
        ctx: &'a mut CtxTransaction,
    ) -> BoxFuture<'a, Result<()>> {
        box_future_ok()
    }

    #[allow(unused_variables)]
    fn track_remove<'a>(
        &'a self,
        key: &'a Self::Key,
        trx: &'a mut CtxTransaction,
    ) -> BoxFuture<'a, Result<()>> {
        box_future_ok()
    }
}

#[cfg(feature = "cache")]
impl<T> Entity for cache::CacheIsland<T>
where
    T: Entity,
{
    type Key = T::Key;
}

impl<T> Entity for Option<T>
where
    T: Entity,
{
    type Key = T::Key;
}

fn box_future_ok() -> BoxFuture<'static, Result<()>> {
    Box::pin(std::future::ready(Ok(())))
}
