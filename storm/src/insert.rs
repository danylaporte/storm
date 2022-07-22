use crate::{BoxFuture, Entity, Result};

pub trait Insert<E: Entity> {
    fn insert<'a>(
        &'a mut self,
        k: E::Key,
        v: E,
        track: &'a E::TrackCtx,
    ) -> BoxFuture<'a, Result<()>>;

    fn insert_all<'a, I>(
        &'a mut self,
        iter: I,
        track: &'a E::TrackCtx,
    ) -> BoxFuture<'a, Result<usize>>
    where
        I: IntoIterator<Item = (E::Key, E)> + Send + 'a,
        I::IntoIter: Send;
}

pub trait InsertIfChanged<E: Entity> {
    fn insert_if_changed<'a>(
        &'a mut self,
        k: E::Key,
        v: E,
        track: &'a E::TrackCtx,
    ) -> BoxFuture<'a, Result<()>>;

    fn insert_all_if_changed<'a, I>(
        &'a mut self,
        iter: I,
        track: &'a E::TrackCtx,
    ) -> BoxFuture<'a, Result<usize>>
    where
        I: IntoIterator<Item = (E::Key, E)> + Send + 'a,
        I::IntoIter: Send;
}

pub trait InsertMut<E: Entity> {
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
    ) -> BoxFuture<'a, Result<usize>>
    where
        I: IntoIterator<Item = (E::Key, E)> + Send + 'a,
        I::IntoIter: Send,
        Self: Send;
}

pub trait InsertMutIfChanged<E: Entity> {
    fn insert_mut_if_changed<'a>(
        &'a mut self,
        k: E::Key,
        v: E,
        track: &'a E::TrackCtx,
    ) -> BoxFuture<'a, Result<E::Key>>;

    fn insert_mut_all_if_changed<'a, I>(
        &'a mut self,
        iter: I,
        track: &'a E::TrackCtx,
    ) -> BoxFuture<'a, Result<usize>>
    where
        I: IntoIterator<Item = (E::Key, E)> + Send + 'a,
        I::IntoIter: Send;
}
