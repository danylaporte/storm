use crate::{BoxFuture, Entity, Result};

pub trait Remove<E: Entity> {
    fn remove(&mut self, k: E::Key) -> BoxFuture<'_, Result<()>>;
}
