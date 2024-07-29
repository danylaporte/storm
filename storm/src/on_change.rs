use crate::{BoxFuture, CtxTransaction, Entity, Result};
use std::sync::Arc;

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
    #[doc(hidden)]
    pub fn __call<'b>(
        &'b self,
        trx: &'b mut CtxTransaction<'_>,
        key: &'b E::Key,
        new: &'b mut E,
        track_ctx: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<()>> {
        let guard = self.0.lock();
        let vec = Arc::clone(&guard);

        drop(guard);

        Box::pin(async move {
            for handler in vec.iter() {
                handler.handle_change(trx, key, new, track_ctx).await?;
            }

            Ok(())
        })
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
