use crate::{Ctx, Entity, Obj, Result, Trx};
use parking_lot::Mutex;
use std::{future::Future, iter::once, ptr::addr_eq, sync::Arc};
use tokio::task_local;

pub type BoxedFut<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;
pub type Fut<'a> = BoxedFut<'a, Result<()>>;

pub type ChangeEvent<E> = Event<
    (dyn for<'a, 'b> Fn(
        &'b mut Trx<'a>,
        &'b <E as Entity>::Key,
        &'b mut E,
        &'b <E as Entity>::TrackCtx,
    ) -> Fut<'b>
         + Sync),
>;

pub type ChangedEvent<E> = Event<
    (dyn for<'a, 'b> Fn(
        &'b mut Trx<'a>,
        &'b <E as Entity>::Key,
        &'b E,
        &'b <E as Entity>::TrackCtx,
    ) -> Fut<'b>
         + Sync),
>;

pub type ClearEvent = Event<(dyn Fn(&mut Ctx) + Sync)>;
pub type LoadedEvent = Event<(dyn for<'a> Fn(&'a Ctx) -> Fut<'a> + Sync)>;

pub type RemoveEvent<Key, Track> =
    Event<(dyn for<'a, 'b> Fn(&'b mut Trx<'a>, &'b Key, &'b Track) -> Fut<'b> + Sync)>;

pub struct Event<T: ?Sized + 'static>(Mutex<Arc<[&'static T]>>);

impl<T: ?Sized + 'static> Default for Event<T> {
    #[inline]
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<T: ?Sized + 'static> Event<T> {
    pub fn list(&self) -> Arc<[&'static T]> {
        Arc::clone(&*self.0.lock())
    }

    pub fn register(&self, item: &'static T) {
        let mut guard = self.0.lock();

        if !guard.iter().any(|a| addr_eq(a, item)) {
            *guard = guard.iter().copied().chain(once(item)).collect();
        }
    }
}

impl<E: Entity> ChangeEvent<E> {
    pub(crate) fn call<'a, 'b>(
        &'b self,
        trx: &'b mut Trx<'a>,
        key: &'b E::Key,
        entity: &'b mut E,
        track: &'b E::TrackCtx,
    ) -> impl Future<Output = Result<()>> + Send + use<'a, 'b, E> {
        CHANGE_DEPTH.scope(change_depth() + 1, async move {
            for h in &*self.list() {
                h(trx, key, entity, track).await?;
            }

            Ok(())
        })
    }
}

impl<E: Entity> ChangedEvent<E> {
    pub(crate) fn call<'a, 'b>(
        &'b self,
        trx: &'b mut Trx<'a>,
        key: &'b E::Key,
        entity: &'b E,
        track: &'b E::TrackCtx,
    ) -> impl Future<Output = Result<()>> + Send + use<'a, 'b, E> {
        CHANGE_DEPTH.scope(change_depth() + 1, async move {
            for h in &*self.list() {
                h(trx, key, entity, track).await?;
            }

            Ok(())
        })
    }
}

impl ClearEvent {
    pub(crate) fn call(&self, ctx: &mut Ctx) {
        for f in &*self.list() {
            f(ctx);
        }
    }

    /// Clear automatically the specified obj when this event is raised.
    pub fn register_clear_obj<A: Obj>(&self) {
        self.register(&clear_obj::<A>);
    }
}

impl LoadedEvent {
    pub(crate) async fn call<'a>(&'a self, ctx: &'a Ctx) -> Result<()> {
        for f in &*self.list() {
            f(ctx).await?;
        }

        Ok(())
    }
}

impl<Key, Track> RemoveEvent<Key, Track> {
    pub(crate) fn call<'a, 'b>(
        &'b self,
        trx: &'b mut Trx<'a>,
        key: &'b Key,
        track: &'b Track,
    ) -> impl Future<Output = Result<()>> + Send + use<'a, 'b, Key, Track>
    where
        Key: Sync,
        Track: Sync,
    {
        CHANGE_DEPTH.scope(change_depth() + 1, async move {
            for h in &*self.list() {
                h(trx, key, track).await?;
            }
            Ok(())
        })
    }
}

fn clear_obj<A: Obj>(ctx: &mut Ctx) {
    ctx.clear::<A>()
}

task_local! {
    static CHANGE_DEPTH: usize;
}

/// Returns a stack depth level 1.. when run inside the on_change event.
/// Each time the event is nested, the depth increase.
/// When called oustide, will returns 0.
pub fn change_depth() -> usize {
    CHANGE_DEPTH.try_with(|v| *v).unwrap_or_default()
}
