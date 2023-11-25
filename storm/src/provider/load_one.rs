use super::LoadArgs;
use crate::{BoxFuture, Entity, Error, GetMut, Result};

pub trait LoadOne<E: Entity>: Send + Sync {
    fn load_one_with_args<'a>(
        &'a self,
        k: &'a E::Key,
        args: LoadArgs,
    ) -> BoxFuture<'a, Result<Option<E>>>;

    fn load_one<'a>(&'a self, k: &'a E::Key) -> BoxFuture<'a, Result<Option<E>>> {
        self.load_one_with_args(k, LoadArgs::default())
    }

    fn load_one_ok<'a>(&'a self, k: &'a E::Key) -> BoxFuture<'a, Result<E>>
    where
        E::Key: std::fmt::Debug,
    {
        Box::pin(async move {
            self.load_one(k)
                .await?
                .ok_or_else(|| Error::load_one_not_found::<_, E>(k))
        })
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

impl<E: Entity> GetMut<E> for LoadNothing
where
    E::Key: PartialEq,
{
    fn get_mut(&mut self, _k: &E::Key) -> Option<&mut E> {
        None
    }
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
