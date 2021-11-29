use crate::{BoxFuture, CtxTransaction, Entity, EntityAccessor, LogAccessor, Result};

pub trait Insertable: Entity + EntityAccessor + LogAccessor {
    fn insertable<'a, 'b>(
        self,
        trx: &'b mut CtxTransaction<'a>,
        k: Self::Key,
        track: &'b Self::TrackCtx,
    ) -> BoxFuture<'b, Result<Self::Key>>
    where
        'a: 'b;
}
