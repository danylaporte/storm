use crate::{Entity, Error, GetMut, Result};
use async_trait::async_trait;

#[async_trait]
pub trait LoadOne<E: Entity> {
    async fn load_one(&self, k: &E::Key) -> Result<Option<E>>;

    async fn load_one_ok(&self, k: &E::Key) -> Result<E> {
        self.load_one(k).await?.ok_or(Error::EntityNotFound)
    }
}

#[doc(hidden)]
#[derive(Default)]
pub struct LoadDoNothing;

impl<T> Extend<T> for LoadDoNothing {
    fn extend<I: IntoIterator<Item = T>>(&mut self, _iter: I) {}
}

#[doc(hidden)]
#[derive(Default)]
pub struct LoadNothing;

impl<T> Extend<T> for LoadNothing {
    fn extend<I: IntoIterator<Item = T>>(&mut self, _iter: I) {}
}

#[doc(hidden)]
pub struct LoadOneInternal<E: Entity>(Option<(E::Key, E)>);

impl<E: Entity> LoadOneInternal<E> {
    pub fn into_inner(self) -> Option<E> {
        self.0.map(|t| t.1)
    }
}

impl<E: Entity> Default for LoadOneInternal<E> {
    fn default() -> Self {
        Self(None)
    }
}

impl<E: Entity> Extend<(E::Key, E)> for LoadOneInternal<E> {
    fn extend<I: IntoIterator<Item = (E::Key, E)>>(&mut self, iter: I) {
        self.0 = iter.into_iter().next();
    }
}

impl<E: Entity> GetMut<E> for LoadOneInternal<E>
where
    E::Key: PartialEq,
{
    fn get_mut(&mut self, k: &E::Key) -> Option<&mut E> {
        self.0.as_mut().filter(|t| &t.0 == k).map(|t| &mut t.1)
    }
}
