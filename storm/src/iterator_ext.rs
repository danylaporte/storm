use crate::{BoxFuture, CtxTransaction, Entity, Insert, InsertMut, Remove, Result};

pub trait IteratorExt: Iterator {
    fn insert_all<'a, 'b, E>(
        self,
        trx: &'b mut CtxTransaction<'a>,
        track: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<usize>>
    where
        CtxTransaction<'a>: Insert<E>,
        E: Entity,
        Self: Iterator<Item = (E::Key, E)> + Sized + Send + 'b,
        'a: 'b,
    {
        trx.insert_all(self, track)
    }

    fn insert_mut_all<'a, 'b, E>(
        self,
        trx: &'b mut CtxTransaction<'a>,
        track: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<usize>>
    where
        CtxTransaction<'a>: InsertMut<E>,
        E: Entity,
        Self: Iterator<Item = (E::Key, E)> + Sized + Send + 'b,
        'a: 'b,
    {
        trx.insert_mut_all(self, track)
    }

    fn remove_all<'a, 'b, E>(
        self,
        trx: &'b mut CtxTransaction<'a>,
        track: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<usize>>
    where
        CtxTransaction<'a>: Remove<E>,
        E: Entity,
        Self: Iterator<Item = E::Key> + Sized + Send + 'b,
        'a: 'b,
    {
        trx.remove_all(self, track)
    }
}

impl<T> IteratorExt for T where T: Iterator {}
