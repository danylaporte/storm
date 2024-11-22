use crate::{Error, Result};
use fxhash::FxHashSet;
use std::{
    any::{type_name, TypeId},
    cell::RefCell,
    future::Future,
    iter::once,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::task_local;

task_local! {
    static FX: RefCell<FxHashSet<TypeId>>;
}

pub(crate) struct CylceDepGuardFut<F>(F, TypeId);

impl<F, T> Future for CylceDepGuardFut<F>
where
    F: Future<Output = Result<T>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe { self.map_unchecked_mut(|this| &mut this.0).poll(cx) }
    }
}

impl<F> Drop for CylceDepGuardFut<F> {
    fn drop(&mut self) {
        remove_id(self.1);
    }
}

fn add_id(id: TypeId, name: &'static str) -> Result<bool> {
    match FX.try_with(|cell| cell.borrow_mut().insert(id)) {
        Ok(true) => Ok(true),
        Ok(false) => Err(Error::CycleDepInit(name)),
        Err(_) => Ok(false),
    }
}

pub(crate) async fn guard<F, Fut, T>(f: F, id: TypeId) -> Result<T>
where
    F: FnOnce(bool) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let name = type_name::<T>();

    if add_id(id, name)? {
        CylceDepGuardFut(f(false), id).await
    } else {
        FX.scope(RefCell::new(once(id).collect()), f(true)).await
    }
}

fn remove_id(id: TypeId) {
    let _ = FX.try_with(|cell| cell.borrow_mut().remove(&id));
}
