use crate::{log::LogToken, Ctx, CtxVars, Gc, LoadedEvent, LogVars, Result, Trx};
use attached::Var;
use std::future::Future;

pub trait Obj: ObjBase + Gc + Send + Sync + Sized + 'static {
    fn ctx_var() -> Var<Self, CtxVars>;
    fn log_var() -> Var<Self::Log, LogVars>;

    fn init(ctx: &Ctx) -> impl Future<Output = Result<Self>> + Send + '_;
    fn loaded() -> &'static LoadedEvent;
}

pub trait ObjBase: Sized + 'static {
    type Log: Default;
    type Trx<'a>;

    fn apply_log(&mut self, log: Self::Log) -> bool;
    fn trx<'a>(&'a self, trx: &'a mut Trx<'a>, log: LogToken<Self::Log>) -> Self::Trx<'a>;
}
