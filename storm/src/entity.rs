use crate::{
    BoxFuture, ChangeEvent, ChangedEvent, ClearEvent, Ctx, CtxVars, Gc, LoadedEvent, LogVars, Obj,
    ObjTrxBase, RemoveEvent, Result, Trx,
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

pub trait EntityObj: Entity + Gc + 'static {
    type Tbl: Obj;

    fn ctx_var() -> Var<Self::Tbl, CtxVars>;
    fn loaded() -> &'static LoadedEvent;
}

pub trait EntityTrx: EntityObj
where
    Self::Tbl: ObjTrxBase,
{
    fn log_var() -> Var<<Self::Tbl as ObjTrxBase>::Log, LogVars>;

    fn change() -> &'static ChangeEvent<Self>;
    fn changed() -> &'static ChangedEvent<Self>;
    fn remove() -> &'static RemoveEvent<Self::Key, Self::TrackCtx>;
    fn removed() -> &'static RemoveEvent<Self::Key, Self::TrackCtx>;

    #[inline]
    fn cleared() -> &'static ClearEvent {
        Ctx::on_clear_obj::<Self::Tbl>()
    }
}
