use crate::{Asset, Ctx, CtxVars, LogVars, Result};
use attached::Var;
use std::future::Future;

pub trait AssetProxy: Sized + Send + 'static {
    type Asset: Asset;

    fn ctx_var() -> Var<Self::Asset, CtxVars>;

    fn log_var() -> Var<<Self::Asset as Asset>::Log, LogVars>;

    fn init(ctx: &Ctx) -> impl Future<Output = Result<Self::Asset>> + Send;
}
