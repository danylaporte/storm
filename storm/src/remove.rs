use crate::{BoxFuture, Entity, Result};

pub trait Remove<E: Entity> {
    fn remove<'a>(&'a mut self, k: E::Key, track: &'a E::TrackCtx) -> BoxFuture<'a, Result<()>>;

    fn remove_all<'a, K>(
        &'a mut self,
        keys: K,
        track: &'a E::TrackCtx,
    ) -> BoxFuture<'a, Result<usize>>
    where
        K: 'a,
        K: IntoIterator<Item = E::Key> + Send,
        K::IntoIter: Send;
}
