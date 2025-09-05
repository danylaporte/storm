use crate::{ApplyLog, Ctx, CtxExt, Entity, HashTable, Result, VecTable};
use async_cell_lock::QueueRwLockQueueGuard;
use extobj::{DynObj, Var, VarId};
use fxhash::FxHashMap;
use std::sync::OnceLock;

pub trait LogOf {
    type Log: Default + Send + Sync + 'static;
}

impl<E: Entity> LogOf for HashTable<E> {
    type Log = TableLog<E>;
}

impl<T: LogOf> LogOf for OnceLock<T> {
    type Log = T::Log;
}

impl<E: Entity> LogOf for VecTable<E> {
    type Log = TableLog<E>;
}

#[derive(Default)]
pub struct Logs(FxHashMap<VarId<CtxExt>, DynObj>);

impl Logs {
    pub async fn apply_log(self, ctx: QueueRwLockQueueGuard<'_, Ctx>) -> Result<bool> {
        Ok(ctx.write().await?.apply_log(self))
    }

    #[inline]
    pub(crate) fn contains<T: LogOf>(&self, var: Var<CtxExt, T>) -> bool {
        self.0.contains_key(&var.var_id())
    }

    #[inline]
    pub(crate) fn get<T: LogOf>(&self, var: Var<CtxExt, T>) -> Option<&T::Log> {
        self.0.get(&var.var_id()).map(|d| unsafe { d.get() })
    }

    #[inline]
    pub(crate) fn get_mut<T: LogOf>(&mut self, var: Var<CtxExt, T>) -> Option<&mut T::Log> {
        self.0
            .get_mut(&var.var_id())
            .map(|d| unsafe { d.get_mut() })
    }

    #[inline]
    pub(crate) fn get_mut_or_default<T: LogOf>(&mut self, var: Var<CtxExt, T>) -> &mut T::Log {
        let d = self
            .0
            .entry(var.var_id())
            .or_insert_with(|| DynObj::new(T::Log::default()));

        unsafe { d.get_mut() }
    }

    #[inline]
    pub(crate) fn insert<T: LogOf>(&mut self, var: Var<CtxExt, T>, log: T::Log) {
        self.0.insert(var.var_id(), DynObj::new(log));
    }

    #[inline]
    pub(crate) fn remove<T: LogOf>(&mut self, var: Var<CtxExt, T>) -> Option<T::Log> {
        self.0
            .remove(&var.var_id())
            .map(|d| unsafe { d.into_inner() })
    }
}

pub type TableLog<E> = FxHashMap<<E as Entity>::Key, Option<E>>;
