use crate::{AssetBase, Ctx, CtxVars, LogVars, Result};
use attached::Var;
use std::future::Future;

pub trait Asset: AssetBase {
    fn ctx_var() -> Var<Self, CtxVars>;
    fn log_var() -> Var<Self::Log, LogVars>;

    fn init(ctx: &Ctx) -> impl Future<Output = Result<Self>> + Send + '_;
}
