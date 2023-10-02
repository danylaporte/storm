use crate::{BoxFuture, CtxTransaction, Entity, Result};
use std::sync::Arc;

pub trait RemovingHandler<E: Entity> {
    fn handle_removing<'a>(
        &'a self,
        trx: &'a mut CtxTransaction<'_>,
        key: &'a E::Key,
        track_ctx: &'a E::TrackCtx,
    ) -> BoxFuture<'a, Result<()>>;
}

impl<E: Entity, T> RemovingHandler<E> for T
where
    T: for<'a> Fn(
        &'a mut CtxTransaction<'_>,
        &'a E::Key,
        &'a E::TrackCtx,
    ) -> BoxFuture<'a, Result<()>>,
{
    fn handle_removing<'a>(
        &'a self,
        trx: &'a mut CtxTransaction<'_>,
        key: &'a <E as Entity>::Key,
        track_ctx: &'a <E as Entity>::TrackCtx,
    ) -> BoxFuture<'a, Result<()>> {
        (self)(trx, key, track_ctx)
    }
}

type ArcRemovingHandler<E> = Arc<dyn RemovingHandler<E> + Send + Sync>;

pub struct OnRemove<E>(parking_lot::Mutex<Arc<Box<[ArcRemovingHandler<E>]>>>);

impl<E: Entity> OnRemove<E> {
    #[doc(hidden)]
    pub fn __call<'b>(
        &'b self,
        trx: &'b mut CtxTransaction<'_>,
        key: &'b E::Key,
        track_ctx: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<()>> {
        let vec = Arc::clone(&self.0.lock());

        Box::pin(async move {
            for handler in vec.iter() {
                handler.handle_removing(trx, key, track_ctx).await?;
            }

            Ok(())
        })
    }

    pub fn register<H: RemovingHandler<E> + Send + Sync + 'static>(&self, handler: H) {
        self.register_impl(Arc::new(handler));
    }

    pub fn register_fn<F>(&self, f: F)
    where
        F: for<'a, 'b> Fn(
                &'b mut CtxTransaction<'a>,
                &'b E::Key,
                &'b E::TrackCtx,
            ) -> BoxFuture<'b, Result<()>>
            + Send
            + Sync
            + 'static,
    {
        self.register(f);
    }

    fn register_impl(&self, handler: ArcRemovingHandler<E>) {
        let mut gate = self.0.lock();
        let mut vec = Vec::with_capacity(gate.len() + 1);

        vec.extend(gate.iter().cloned());
        vec.push(handler);

        *gate = Arc::new(vec.into_boxed_slice());
    }
}

impl<E> Default for OnRemove<E> {
    fn default() -> Self {
        Self(Default::default())
    }
}
