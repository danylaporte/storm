use crate::{Entity, Result};
use async_trait::async_trait;

#[async_trait]
pub trait LoadOne<E: Entity> {
    async fn load_one(&self, k: &E::Key) -> Result<Option<E>>;
}

#[doc(hidden)]
pub struct LoadOneInternal<E>(Option<E>);

impl<E> LoadOneInternal<E> {
    pub fn into_inner(self) -> Option<E> {
        self.0
    }
}

impl<E> Default for LoadOneInternal<E> {
    fn default() -> Self {
        Self(None)
    }
}

impl<E: Entity> Extend<(E::Key, E)> for LoadOneInternal<E> {
    fn extend<I: IntoIterator<Item = (E::Key, E)>>(&mut self, iter: I) {
        self.0 = iter.into_iter().next().map(|t| t.1);
    }
}
