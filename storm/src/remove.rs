use crate::{BoxFuture, EntityObj, Result};

pub trait Remove<E: EntityObj>: Send {
    fn remove<'a>(&'a mut self, k: E::Key, track: &'a E::TrackCtx) -> BoxFuture<'a, Result<()>>;

    fn remove_all<'a, K>(&'a mut self, keys: K, track: &'a E::TrackCtx) -> BoxFuture<'a, Result<()>>
    where
        K: IntoIterator<Item = E::Key> + Send + 'a,
        K::IntoIter: Send,
    {
        Box::pin(async move {
            for id in keys {
                self.remove(id, track).await?;
            }

            Ok(())
        })
    }

    fn remove_filter<'a, F>(
        &'a mut self,
        mut filter: F,
        track: &'a E::TrackCtx,
    ) -> BoxFuture<'a, Result<()>>
    where
        E::Key: Clone,
        F: FnMut(&E::Key, &E) -> bool,
        for<'c> &'c Self: IntoIterator<Item = (&'c E::Key, &'c E)>,
    {
        let ids = self
            .into_iter()
            .filter(|t| filter(t.0, t.1))
            .map(|t| (t.0.clone()))
            .collect::<Vec<E::Key>>();

        self.remove_all(ids, track)
    }
}
