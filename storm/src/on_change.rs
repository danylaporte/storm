use crate::{BoxFuture, CtxTransaction, Entity, Result};
use std::sync::Arc;
use tokio::task_local;

pub trait ChangeHandler<E: Entity> {
    fn handle_change<'a>(
        &'a self,
        trx: &'a mut CtxTransaction<'_>,
        key: &'a E::Key,
        new: &'a mut E,
        track_ctx: &'a E::TrackCtx,
    ) -> BoxFuture<'a, Result<()>>;
}

impl<E: Entity, T> ChangeHandler<E> for T
where
    T: for<'a> Fn(
        &'a mut CtxTransaction<'_>,
        &'a E::Key,
        &'a mut E,
        &'a E::TrackCtx,
    ) -> BoxFuture<'a, Result<()>>,
{
    fn handle_change<'a>(
        &'a self,
        trx: &'a mut CtxTransaction<'_>,
        key: &'a <E as Entity>::Key,
        new: &'a mut E,
        track_ctx: &'a <E as Entity>::TrackCtx,
    ) -> BoxFuture<'a, Result<()>> {
        (self)(trx, key, new, track_ctx)
    }
}

type ArcChangeHandler<E> = Arc<dyn ChangeHandler<E> + Send + Sync>;

pub struct OnChange<E>(parking_lot::Mutex<Arc<Box<[ArcChangeHandler<E>]>>>);

impl<E: Entity> OnChange<E> {
    pub(crate) fn call<'b>(
        &'b self,
        trx: &'b mut CtxTransaction<'_>,
        key: &'b E::Key,
        new: &'b mut E,
        track_ctx: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<()>> {
        let vec = {
            let guard = self.0.lock();
            Arc::clone(&guard)
        };

        let block = async move {
            for handler in vec.iter() {
                handler.handle_change(trx, key, new, track_ctx).await?;
            }
            Ok(())
        };

        Box::pin(CHANGE_DEPTH.scope(change_depth() + 1, block))
    }

    pub fn register<H: ChangeHandler<E> + Send + Sync + 'static>(&self, handler: H) {
        self.register_impl(Arc::new(handler));
    }

    pub fn register_fn<F>(&self, f: F)
    where
        F: for<'a, 'b> Fn(
                &'b mut CtxTransaction<'a>,
                &'b E::Key,
                &'b mut E,
                &'b E::TrackCtx,
            ) -> BoxFuture<'b, Result<()>>
            + Send
            + Sync
            + 'static,
    {
        self.register(f);
    }

    fn register_impl(&self, handler: ArcChangeHandler<E>) {
        let mut gate = self.0.lock();
        let mut vec = Vec::with_capacity(gate.len() + 1);

        vec.extend(gate.iter().cloned());
        vec.push(handler);

        *gate = Arc::new(vec.into_boxed_slice());
    }
}

impl<E> Default for OnChange<E> {
    fn default() -> Self {
        Self(Default::default())
    }
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
