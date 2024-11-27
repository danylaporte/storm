use crate::{Ctx, CtxVars, LogVars, ObjBase, Result};
use attached::Var;
use std::future::Future;

pub trait Obj: ObjBase {
    fn ctx_var() -> Var<Self, CtxVars>;
    fn log_var() -> Var<Self::Log, LogVars>;

    fn init(ctx: &Ctx) -> impl Future<Output = Result<Self>> + Send + '_;
}
