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
        clear_deps(Self::deps(), ctx);
    }

    fn register_deps<F: Fn(&mut Vars) + Send + Sync + 'static>(f: F) {
        register_deps(Self::deps(), Box::new(f));
    }
}

fn clear_deps(deps: &'static Deps, ctx: &mut Vars) {
    deps.read().iter().for_each(|f| f(ctx));
}

fn register_deps(deps: &'static Deps, f: Box<dyn Fn(&mut Vars) + Send + Sync + 'static>) {
    deps.write().push(f);
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
pub type LogsVar = attached::Vars<vars::Log>;
pub type Vars = attached::Vars<vars::Tbl>;

pub(crate) mod vars {
    use attached::var_ctx;

    var_ctx!(pub Tbl);
    var_ctx!(pub Log);
}
