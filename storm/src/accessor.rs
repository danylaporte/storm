use crate::{indexing::{Index, IndexList}, logs::LogId, CtxTransaction, Entity, Inits, Log, LogState, Logs, OnChange, OnChanged, OnRemove};
use extobj::{extobj, ExtObj, Var};
use parking_lot::RwLock;
use std::{any::Any, collections::hash_map::Entry, sync::OnceLock};

pub type Deps = RwLock<Vec<Box<dyn Fn(&mut CtxExtObj) + Send + Sync>>>;
pub type CtxVar<T> = Var<CtxExt, OnceLock<T>>;
pub type LogVar<T> = Var<LogExt, OnceLock<T>>;

extobj!(pub struct CtxExt);

pub type CtxExtObj = ExtObj<CtxExt>;

pub trait Accessor: Sized + 'static {
    fn var() -> CtxVar<Self>;
    fn deps() -> &'static Deps;

    fn clear(ctx: &mut CtxExtObj) {
        // clear the variable.
        ctx.get_mut(Self::var()).take();
        Self::clear_deps(ctx);
    }

    fn clear_deps(ctx: &mut CtxExtObj) {
        clear_deps(Self::deps(), ctx);
    }

    fn register_deps<F: Fn(&mut CtxExtObj) + Send + Sync + 'static>(f: F) {
        register_deps(Self::deps(), Box::new(f));
    }
}

fn clear_deps(deps: &'static Deps, ctx: &mut CtxExtObj) {
    deps.read().iter().for_each(|f| f(ctx));
}

fn register_deps(deps: &'static Deps, f: Box<dyn Fn(&mut CtxExtObj) + Send + Sync + 'static>) {
    deps.write().push(f);
}

pub trait EntityAccessor: Entity + Sized + 'static {
    type Tbl: Send + Sync;

    fn ctx_var() -> CtxVar<Self::Tbl>;

    fn log_id() -> LogId;

    fn log_mut(logs: &mut Logs) -> &mut Log<Self> {
        Any::downcast_mut(&**match logs.0.entry(Self::log_id()) {
            Entry::Occupied(o) =>  o.into_mut(),
            Entry::Vacant(v) => v.insert(Box::new(Log::default()))
        })
    }

    fn log_ref(logs: &Logs) -> Option<&Log<Self>> {
        logs.0.get(&Self::log_id()).map(|v| Any::downcast_ref(&**v))
    }

    fn register_index()

    fn entity_deps() -> &'static Deps;

    fn entity_inits() -> &'static Inits<Self::Tbl>;

    fn on_change() -> &'static OnChange<Self>;

    fn on_changed() -> &'static OnChanged<Self>;

    fn on_remove() -> &'static OnRemove<Self>;
}

pub struct TblIndexRegistry<E>(std::sync::Mutex<Vec<fn() -> Box<dyn TblIndex<E>>>>);

pub trait TblIndexAddr {
    fn id() -> usize;
}

impl<E> TblIndexRegistry<E> {
    pub fn register(&self, init: fn() -> Box<dyn TblIndex<E>>) -> u32 {
        let mut gate = self.0.lock().unwrap();
        let id = gate.len();

        gate.push(init);
        
        id
    }

    pub fn create_tbl_index_list(&self) -> IndexList<E> {
        let gate = self.0.lock().unwrap();
    }
}

pub struct TblIndexList<E>(Vec<usize>);

impl<E> TblIndexList<E> {
    #[inline]
    pub fn get<I>(&self) -> &TblIndex<E> where I: TblIndex + TblIndexAddr {
        let id = I::id();
        unsafe { self.0.get_unchecked(I::id()) }
    }

    #[inline]
    pub fn get_mut<I>(&mut self) -> &mut TblIndex<E> where I: TblIndex + TblIndexAddr {
        let id = I::id();
        unsafe  { self.0.get_unchecked_mut(id) }
    }
}

pub struct TblIndexLog<E>