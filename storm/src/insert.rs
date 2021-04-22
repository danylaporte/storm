use crate::{BoxFuture, Entity, Result};

pub trait Insert<E: Entity> {
    fn insert(&mut self, k: E::Key, v: E) -> BoxFuture<'_, Result<()>>;
}
