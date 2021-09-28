use crate::{BoxFuture, Entity, Result};

pub trait Remove<E: Entity> {
    fn remove<'a>(&'a mut self, k: E::Key, tracker: &'a E::TrackCtx) -> BoxFuture<'a, Result<()>>;

    fn remove_all<'a, K>(
        &'a mut self,
        keys: K,
        tracker: &'a E::TrackCtx,
    ) -> BoxFuture<'a, Result<()>>
    where
        K: 'a,
        K: IntoIterator<Item = E::Key> + Send,
        K::IntoIter: Send,
        Self: Send,
    {
        Box::pin(async move {
            for key in keys {
                self.remove(key, tracker).await?;
            }

            Ok(())
        })
    }
}
