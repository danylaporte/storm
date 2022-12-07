use crate::{BoxFuture, CtxTransaction, Entity, Result};
use std::sync::Arc;

pub trait RemovingHandler<E: Entity> {
    fn handle_removing<'a, 'b>(
        &'b self,
        trx: &'b mut CtxTransaction<'a>,
        key: &'b E::Key,
        track_ctx: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<()>>;
}

impl<E: Entity, T> RemovingHandler<E> for T
where
    T: for<'a, 'b> Fn(
        &'b mut CtxTransaction<'a>,
        &'b E::Key,
        &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<()>>,
{
    fn handle_removing<'a, 'b>(
        &'b self,
        trx: &'b mut CtxTransaction<'a>,
        key: &'b <E as Entity>::Key,
        track_ctx: &'b <E as Entity>::TrackCtx,
    ) -> BoxFuture<'b, Result<()>> {
        (self)(trx, key, track_ctx)
    }
}

type ArcRemovingHandler<E> = Arc<dyn RemovingHandler<E> + Send + Sync>;

pub struct OnRemove<E>(parking_lot::Mutex<Arc<Box<[ArcRemovingHandler<E>]>>>);

impl<E: Entity> OnRemove<E> {
    #[doc(hidden)]
    pub fn __call<'a, 'b>(
        &'b self,
        trx: &'b mut CtxTransaction<'a>,
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

    pub fn register<EV: RemovingHandler<E> + Send + Sync + 'static>(&self, ev: EV) {
        let mut gate = self.0.lock();

        let vec = gate
            .iter()
            .map(Arc::clone)
            .chain(std::iter::once(Arc::new(ev) as _))
            .collect::<Vec<_>>()
            .into_boxed_slice();

        *gate = Arc::new(vec);
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
}

impl<E> Default for OnRemove<E> {
    fn default() -> Self {
        Self(Default::default())
    }
}
