use crate::{Entity, Log};
use attached::Var;
use parking_lot::RwLock;

pub type Deps = RwLock<Vec<Box<dyn Fn(&mut Vars) + Send + Sync>>>;
pub type LogVar<T> = Var<T, vars::Log>;
pub type TblVar<T> = Var<T, vars::Tbl>;

pub trait Accessor: Sized + 'static {
    fn var() -> &'static TblVar<Self>;
    fn deps() -> &'static Deps;

    fn clear(ctx: &mut Vars) {
        ctx.clear(Self::var());
        Self::clear_deps(ctx);
    }

    fn clear_deps(ctx: &mut Vars) {
        Self::deps().read().iter().for_each(|f| f(ctx));
    }

    fn register_deps<F: Fn(&mut Vars) + Send + Sync + 'static>(f: F) {
        Self::deps().write().push(Box::new(f));
    }
}

pub trait EntityAccessor: Sized + 'static {
    type Tbl: Send + Sync;

    fn entity_var() -> &'static TblVar<Self::Tbl>;

    fn entity_deps() -> &'static Deps;
}

pub trait LogAccessor: Entity + Sized + 'static {
    fn log_var() -> &'static LogVar<Log<Self>>;
}

// typed variable contexts
pub type Logs = attached::Vars<vars::Log>;
pub type Vars = attached::Vars<vars::Tbl>;

pub mod vars {
    use attached::var_ctx;

    var_ctx!(pub Tbl);
    var_ctx!(pub Log);
}
