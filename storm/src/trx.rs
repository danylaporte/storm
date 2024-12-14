use crate::{
    provider::TransactionProvider, BoxFuture, Ctx, EntityObj, GetOwned, Insert, InsertMut, Log,
    LogToken, Obj, ObjBase, Remove, Result, TrxErrGate,
};
use std::{borrow::Cow, mem::transmute};

pub struct Trx<'a> {
    pub ctx: &'a Ctx,
    pub(crate) provider: TransactionProvider<'a>,
    pub log: Log,
    pub(crate) err_gate: TrxErrGate,
}

impl<'a> Trx<'a> {
    pub async fn commit(self) -> Result<Log> {
        self.err_gate.check()?;
        self.provider.commit().await?;
        Ok(self.log)
    }

    #[inline]
    pub async fn get_entity<'b, E, Q>(&'b mut self, q: &Q) -> Result<Option<&'b E>>
    where
        E: EntityObj,
        <E::Tbl as ObjBase>::Trx<'b>: GetOwned<'b, E, Q>,
    {
        self.tbl_of::<E>().await.map(|t| t.get_owned(q))
    }

    pub fn insert<'b, E>(
        &'b mut self,
        id: E::Key,
        entity: E,
        track: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<()>>
    where
        'a: 'b,
        E: EntityObj,
        for<'c> <E::Tbl as ObjBase>::Trx<'c>: Insert<E>,
    {
        Box::pin(async move { self.obj::<E::Tbl>().await?.insert(id, entity, track).await })
    }

    #[inline]
    pub async fn insert_mut<'b, E>(
        &'b mut self,
        id: E::Key,
        entity: E,
        track: &E::TrackCtx,
    ) -> Result<E::Key>
    where
        E: EntityObj,
        for<'c> <E::Tbl as ObjBase>::Trx<'c>: InsertMut<E>,
    {
        self.obj::<E::Tbl>()
            .await?
            .insert_mut(id, entity, track)
            .await
    }

    pub fn obj<A: Obj>(&mut self) -> BoxFuture<'_, Result<A::Trx<'_>>> {
        Box::pin(async move {
            let obj = self.ctx.obj::<A>().await?;
            Ok(obj.trx(coerce(self), LogToken::from_obj::<A>()))
        })
    }

    pub fn obj_opt<A: Obj>(&mut self) -> Option<A::Trx<'_>> {
        let obj = self.ctx.obj_opt::<A>()?;
        Some(obj.trx(coerce(self), LogToken::from_obj::<A>()))
    }

    #[inline]
    pub fn provider(&self) -> &TransactionProvider<'a> {
        &self.provider
    }

    #[inline]
    pub async fn remove<'b, E>(&'b mut self, id: E::Key, track: &E::TrackCtx) -> Result<()>
    where
        E: EntityObj,
        for<'c> <E::Tbl as ObjBase>::Trx<'c>: Remove<E>,
    {
        self.obj::<E::Tbl>().await?.remove(id, track).await
    }

    #[inline]
    pub fn remove_filter<'b, E, F>(
        &'b mut self,
        filter: F,
        track: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<()>>
    where
        for<'c> <E::Tbl as ObjBase>::Trx<'c>: Remove<E>,
        E: EntityObj,
        E::Key: Clone,
        F: FnMut(&E::Key, &E) -> bool + Send + 'b,
        for<'c, 'd> &'d <E::Tbl as ObjBase>::Trx<'c>: IntoIterator<Item = (&'d E::Key, &'d E)>,
    {
        Box::pin(async move {
            self.obj::<E::Tbl>()
                .await?
                .remove_filter(filter, track)
                .await
        })
    }

    #[inline]
    pub fn tbl_of<E: EntityObj>(&mut self) -> BoxFuture<'_, Result<<E::Tbl as ObjBase>::Trx<'_>>> {
        self.obj::<E::Tbl>()
    }

    #[inline]
    pub fn update_with<'b, E, F>(
        &'b mut self,
        updater: F,
        track: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<()>>
    where
        E: EntityObj + ToOwned<Owned = E>,
        E::Key: Clone,
        F: for<'c> FnMut(&'c E::Key, &'c mut Cow<E>) + Send + 'b,
        for<'c, 'd> &'d <E::Tbl as ObjBase>::Trx<'c>: IntoIterator<Item = (&'d E::Key, &'d E)>,
        for<'c> <E::Tbl as ObjBase>::Trx<'c>: Insert<E>,
    {
        Box::pin(async move {
            let mut obj = self.obj::<E::Tbl>().await?;
            obj.update_with(updater, track).await
        })
    }

    #[inline]
    pub fn update_mut_with<'b, E, F>(
        &'b mut self,
        updater: F,
        track: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<()>>
    where
        E: EntityObj + ToOwned<Owned = E>,
        E::Key: Clone,
        F: for<'c> FnMut(&'c E::Key, &'c mut Cow<E>) + Send + 'b,
        for<'c, 'd> &'d <E::Tbl as ObjBase>::Trx<'c>: IntoIterator<Item = (&'d E::Key, &'d E)>,
        for<'c> <E::Tbl as ObjBase>::Trx<'c>: InsertMut<E>,
    {
        Box::pin(async move {
            self.obj::<E::Tbl>()
                .await?
                .update_mut_with(updater, track)
                .await
        })
    }
}

/// coerce the lifetime of the transaction. This is safe because all entities must be 'static.
fn coerce<'a>(trx: &'a mut Trx<'_>) -> &'a mut Trx<'a> {
    unsafe { transmute(trx) }
}
