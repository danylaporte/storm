use crate::{Entity, Result};
use async_trait::async_trait;

#[async_trait]
pub trait LoadAll<E: Entity, FILTER: Send + Sync, C>
where
    C: Default + Extend<(E::Key, E)> + Send,
{
    async fn load_all(&self, filter: &FILTER) -> Result<C>;
}

#[async_trait]
impl<C, E, FILTER, P> LoadAll<E, FILTER, C> for &P
where
    C: Default + Extend<(E::Key, E)> + Send + 'static,
    E: Entity + 'static,
    FILTER: Send + Sync,
    P: LoadAll<E, FILTER, C> + Send + Sync,
{
    async fn load_all(&self, filter: &FILTER) -> Result<C> {
        (**self).load_all(filter).await
    }
}

pub struct LoadAllKeyOnly<E: Entity>(Vec<E::Key>);

impl<E: Entity> LoadAllKeyOnly<E> {
    pub fn into_inner(self) -> Vec<E::Key> {
        self.0
    }
}

impl<E: Entity> Default for LoadAllKeyOnly<E> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<E: Entity> Extend<(E::Key, E)> for LoadAllKeyOnly<E> {
    fn extend<T: IntoIterator<Item = (E::Key, E)>>(&mut self, iter: T) {
        self.0.extend(iter.into_iter().map(|t| t.0))
    }
}
