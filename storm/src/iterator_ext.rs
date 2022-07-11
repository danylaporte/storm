use crate::{
    AsRefAsync, BoxFuture, Ctx, CtxTransaction, Entity, EntityAccessor, Insert, InsertMut,
    LogAccessor, Remove, Result, TblTransaction,
};

pub trait IteratorExt: Iterator {
    fn insert_all<'a, 'b, E>(
        self,
        trx: &'b mut CtxTransaction<'a>,
        track: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<usize>>
    where
        Ctx: AsRefAsync<E::Tbl>,
        E: Entity + EntityAccessor + LogAccessor,
        Self: Iterator<Item = (E::Key, E)> + Sized + Send + 'b,
        TblTransaction<'a, 'b, E>: Insert<E>,
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
        Ctx: AsRefAsync<E::Tbl>,
        E: Entity + EntityAccessor + LogAccessor,
        Self: Iterator<Item = (E::Key, E)> + Sized + Send + 'b,
        TblTransaction<'a, 'b, E>: InsertMut<E>,
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
        Ctx: AsRefAsync<E::Tbl>,
        E: Entity + EntityAccessor + LogAccessor,
        Self: Iterator<Item = E::Key> + Sized + Send + 'b,
        TblTransaction<'a, 'b, E>: Remove<E>,
        'a: 'b,
    {
        trx.remove_all(self, track)
    }
}

impl<T> IteratorExt for T where T: Iterator {}
