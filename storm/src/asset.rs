use crate::{Ctx, CtxVars, LogVars, Result, Trx};
use attached::Var;
use std::future::Future;

/// A table or an index in the storm Ctx is an asset.
pub trait Asset: Sized + Send + 'static {
    const SUPPORT_GC: bool = false;

    type Log: Default;
    type Trx<'a: 'b, 'b>;

    fn apply_log(&mut self, log: Self::Log) -> bool;
    fn ctx_var() -> Var<Self, CtxVars>;
    fn gc(&mut self);
    fn init(ctx: &Ctx) -> impl Future<Output = Result<Self>> + Send;
    fn log_var() -> Var<Self::Log, LogVars>;

    fn trx<'a: 'b, 'b>(
        trx: &'b mut Trx<'a>,
    ) -> impl Future<Output = Result<Self::Trx<'a, 'b>>> + 'b;

    fn trx_opt<'a: 'b, 'b>(trx: &'b mut Trx<'a>) -> Option<Self::Trx<'a, 'b>>;
}
