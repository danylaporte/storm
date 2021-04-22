use crate::{Entity, Log};
use attached::Var;
use parking_lot::RwLock;

pub type Deps = RwLock<Vec<Box<dyn Fn(&mut VarCtx) + Send + Sync>>>;
pub type LogVar<T> = Var<T, vars::Log>;
pub type TblVar<T> = Var<T, vars::Tbl>;

pub trait Accessor: Sized + 'static {
    fn var() -> &'static TblVar<Self>;
    fn deps() -> &'static Deps;

    fn clear(ctx: &mut VarCtx) {
        Self::var().take(ctx);
        Self::clear_deps(ctx);
    }

    fn clear_deps(ctx: &mut VarCtx) {
        Self::deps().read().iter().for_each(|f| f(ctx));
    }

    fn register_deps<F: Fn(&mut VarCtx) + Send + Sync + 'static>(f: F) {
        Self::deps().write().push(Box::new(f));
    }
}

pub trait EntityAccessor: Sized + 'static {
    type Coll;

    fn entity_var() -> &'static TblVar<Self::Coll>;

    fn entity_deps() -> &'static Deps;
}

pub trait LogAccessor: Entity + Sized + 'static {
    fn log_var() -> &'static LogVar<Log<Self>>;
}

// typed variable contexts
pub type LogCtx = attached::VarCtx<vars::Log>;
pub type VarCtx = attached::VarCtx<vars::Tbl>;

pub mod vars {
    use attached::var_ctx;

    var_ctx!(pub Tbl);
    var_ctx!(pub Log);
}
