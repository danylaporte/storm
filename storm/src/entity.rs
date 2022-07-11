use crate::{BoxFuture, CtxTransaction, Result};
use std::fmt::Debug;

pub trait Entity: Send + Sync + 'static {
    type Key: Send + Sync;
    type TrackCtx: Debug + Sync;

    fn track_insert<'a>(
        &'a self,
        _key: &'a Self::Key,
        _old: Option<&'a Self>,
        _ctx: &'a mut CtxTransaction,
        _tracker: &'a Self::TrackCtx,
    ) -> BoxFuture<'a, Result<()>> {
        box_future_ok()
    }

    fn track_remove<'a>(
        &'a self,
        _key: &'a Self::Key,
        _ctx: &'a mut CtxTransaction,
        _tracker: &'a Self::TrackCtx,
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
    type TrackCtx = T::TrackCtx;
}

impl<T> Entity for Option<T>
where
    T: Entity,
{
    type Key = T::Key;
    type TrackCtx = T::TrackCtx;
}

fn box_future_ok() -> BoxFuture<'static, Result<()>> {
    Box::pin(std::future::ready(Ok(())))
}
