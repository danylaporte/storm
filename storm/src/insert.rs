use crate::{BoxFuture, Entity, Result};

pub trait Insert<E: Entity> {
    fn insert<'a>(
        &'a mut self,
        k: E::Key,
        v: E,
        track: &'a E::TrackCtx,
    ) -> BoxFuture<'a, Result<E::Key>>;

    fn insert_all<'a, I>(&'a mut self, iter: I, track: &'a E::TrackCtx) -> BoxFuture<'a, Result<()>>
    where
        I: IntoIterator<Item = (E::Key, E)> + Send + 'a,
        I::IntoIter: Send,
        Self: Send,
    {
        Box::pin(async move {
            for (k, v) in iter {
                self.insert(k, v, track).await?;
            }

            Ok(())
        })
    }
}
