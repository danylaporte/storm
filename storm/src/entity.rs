use crate::{
    BoxFuture, ChangeEvent, ChangedEvent, ClearEvent, Ctx, CtxVars, Gc, LoadedEvent, LogVars, Obj,
    ObjBase, RemoveEvent, Result, Trx,
};
use attached::Var;
use std::fmt::Debug;

pub trait Entity: Send + Sync + 'static {
    type Key: Send + Sync;
    type TrackCtx: Debug + Send + Sync;

    fn track_insert<'a>(
        &'a self,
        _key: &'a Self::Key,
        _old: Option<&'a Self>,
        _trx: &'a mut Trx,
        _track: &'a Self::TrackCtx,
    ) -> BoxFuture<'a, Result<()>> {
        box_future_ok()
    }

    fn track_remove<'a>(
        &'a self,
        _key: &'a Self::Key,
        _trx: &'a mut Trx,
        _track: &'a Self::TrackCtx,
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

pub trait EntityObj: Entity + Gc + PartialEq + 'static {
    type Tbl: Obj;

    fn change() -> &'static ChangeEvent<Self>;
    fn changed() -> &'static ChangedEvent<Self>;
    fn ctx_var() -> Var<Self::Tbl, CtxVars>;
    fn log_var() -> Var<<Self::Tbl as ObjBase>::Log, LogVars>;
    fn remove() -> &'static RemoveEvent<Self::Key, Self::TrackCtx>;
    fn removed() -> &'static RemoveEvent<Self::Key, Self::TrackCtx>;

    #[inline]
    fn cleared() -> &'static ClearEvent {
        Ctx::on_clear_obj::<Self::Tbl>()
    }

    fn loaded() -> &'static LoadedEvent;
}
