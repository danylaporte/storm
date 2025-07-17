use crate::{indexing::IndexList, Entity, Inits, Log, OnChange, OnChanged, OnRemove};
use attached::Var;
use parking_lot::RwLock;

pub type Deps = RwLock<Vec<Box<dyn Fn(&mut Vars) + Send + Sync>>>;
pub type LogVar<T> = Var<T, vars::Log>;
pub type TblVar<T> = Var<T, vars::Tbl>;

pub trait Accessor: Sized + 'static {
    fn var() -> TblVar<Self>;
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

pub trait EntityAccessor: Entity + Sized + 'static {
    type Tbl: AsRef<IndexList<Self>> + Send + Sync;

    fn entity_var() -> TblVar<Self::Tbl>;

    fn entity_deps() -> &'static Deps;

    fn entity_inits() -> &'static Inits<Self::Tbl>;

    fn on_change() -> &'static OnChange<Self>;

    fn on_changed() -> &'static OnChanged<Self>;

    fn on_remove() -> &'static OnRemove<Self>;
}

pub trait LogAccessor: Entity + Sized + 'static {
    fn log_var() -> LogVar<Log<Self>>;
}

// typed variable contexts
pub type LogsVar = attached::Container<vars::Log>;
pub type Vars = attached::Container<vars::Tbl>;

pub mod vars {
    use attached::container;

    container!(pub Tbl);
    container!(pub Log);
}
