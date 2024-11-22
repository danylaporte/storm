use crate::{BoxFuture, Entity, Result};
use std::borrow::Cow;

pub trait Insert<E: Entity>: Send {
    fn insert<'a>(
        &'a mut self,
        k: E::Key,
        v: E,
        track: &'a E::TrackCtx,
    ) -> BoxFuture<'a, Result<()>>;

    fn insert_all<'a, I>(&'a mut self, iter: I, track: &'a E::TrackCtx) -> BoxFuture<'a, Result<()>>
    where
        I: IntoIterator<Item = (E::Key, E)> + Send + 'a,
        I::IntoIter: Send,
        Self: Send,
    {
        Box::pin(async move {
            for (id, entity) in iter {
                self.insert(id, entity, track).await?;
            }

            Ok(())
        })
    }

    fn update_with<'a, F>(
        &'a mut self,
        mut updater: F,
        track: &'a E::TrackCtx,
    ) -> BoxFuture<'a, Result<()>>
    where
        E: PartialEq + ToOwned<Owned = E>,
        E::Key: Clone,
        F: for<'c> FnMut(&'c E::Key, &'c mut Cow<E>),
        for<'c> &'c Self: IntoIterator<Item = (&'c E::Key, &'c E)>,
        Self: Send,
    {
        let vec = self
            .into_iter()
            .filter_map(|(id, e)| {
                let mut e = Cow::Borrowed(e);
                updater(id, &mut e);

                if let Cow::Owned(e) = e {
                    Some((id.clone(), e))
                } else {
                    None
                }
            })
            .collect::<Vec<(E::Key, E)>>();

        self.insert_all(vec, track)
    }
}

pub trait InsertMut<E: Entity>: Send {
    fn insert_mut<'a>(
        &'a mut self,
        k: E::Key,
        v: E,
        track: &'a E::TrackCtx,
    ) -> BoxFuture<'a, Result<E::Key>>;

    fn insert_mut_all<'a, I>(
        &'a mut self,
        iter: I,
        track: &'a E::TrackCtx,
    ) -> BoxFuture<'a, Result<()>>
    where
        I: IntoIterator<Item = (E::Key, E)> + Send + 'a,
        I::IntoIter: Send,
        Self: Send,
    {
        Box::pin(async move {
            for (id, entity) in iter {
                self.insert_mut(id, entity, track).await?;
            }

            Ok(())
        })
    }

    fn update_mut_with<'a, F>(
        &'a mut self,
        mut updater: F,
        track: &'a E::TrackCtx,
    ) -> BoxFuture<'a, Result<()>>
    where
        E: PartialEq + ToOwned<Owned = E>,
        E::Key: Clone,
        F: for<'c> FnMut(&'c E::Key, &'c mut Cow<E>),
        for<'c> &'c Self: IntoIterator<Item = (&'c E::Key, &'c E)>,
        Self: Send,
    {
        let vec = self
            .into_iter()
            .filter_map(|(id, e)| {
                let mut e = Cow::Borrowed(e);
                updater(id, &mut e);

                if let Cow::Owned(e) = e {
                    Some((id.clone(), e))
                } else {
                    None
                }
            })
            .collect::<Vec<(E::Key, E)>>();

        self.insert_mut_all(vec, track)
    }
}
