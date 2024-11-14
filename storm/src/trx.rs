use std::borrow::Cow;

use crate::{
    provider::TransactionProvider, Asset, Ctx, EntityAsset, GetOwned, Insert, InsertMut, Log,
    Remove, Result, TrxErrGate,
};

pub struct Trx<'a> {
    pub ctx: &'a Ctx,
    pub(crate) provider: TransactionProvider<'a>,
    pub log: Log,
    pub(crate) err_gate: TrxErrGate,
}

impl<'a> Trx<'a> {
    #[inline]
    pub async fn asset<'b, A: Asset>(&'b mut self) -> Result<A::Trx<'a, 'b>> {
        A::trx(self).await
    }

    #[inline]
    pub fn asset_opt<'b, A: Asset>(&'b mut self) -> Option<A::Trx<'a, 'b>> {
        A::trx_opt(self)
    }

    pub async fn commit(self) -> Result<Log> {
        self.err_gate.check()?;
        self.provider.commit().await?;
        Ok(self.log)
    }

    #[inline]
    pub async fn get_entity<'b, E, Q>(&'b mut self, q: &Q) -> Result<Option<&'b E>>
    where
        E: EntityAsset,
        <E::Tbl as Asset>::Trx<'a, 'b>: GetOwned<'b, E, Q>,
    {
        self.tbl_of::<E>().await.map(|t| t.get_owned(q))
    }

    pub async fn insert<'b, E>(
        &'b mut self,
        id: E::Key,
        entity: E,
        track: &E::TrackCtx,
    ) -> Result<()>
    where
        E: EntityAsset + PartialEq,
        <E::Tbl as Asset>::Trx<'a, 'b>: Insert<E>,
    {
        self.asset::<E::Tbl>()
            .await?
            .insert(id, entity, track)
            .await
    }

    #[inline]
    pub async fn insert_mut<'b, E>(
        &'b mut self,
        id: E::Key,
        entity: E,
        track: &E::TrackCtx,
    ) -> Result<E::Key>
    where
        E: EntityAsset + PartialEq,
        <E::Tbl as Asset>::Trx<'a, 'b>: InsertMut<E>,
    {
        self.asset::<E::Tbl>()
            .await?
            .insert_mut(id, entity, track)
            .await
    }

    #[inline]
    pub fn provider(&self) -> &TransactionProvider<'a> {
        &self.provider
    }

    #[inline]
    pub async fn remove<'b, E>(&'b mut self, id: E::Key, track: &E::TrackCtx) -> Result<()>
    where
        E: EntityAsset,
        <E::Tbl as Asset>::Trx<'a, 'b>: Remove<E>,
    {
        self.asset::<E::Tbl>().await?.remove(id, track).await
    }

    #[inline]
    pub async fn tbl_of<'b, E: EntityAsset>(
        &'b mut self,
    ) -> Result<<E::Tbl as Asset>::Trx<'a, 'b>> {
        self.asset::<E::Tbl>().await
    }

    #[inline]
    pub async fn update_with<'b, E, F>(&'b mut self, updater: F, track: &E::TrackCtx) -> Result<()>
    where
        E: EntityAsset + PartialEq + ToOwned<Owned = E>,
        E::Key: Clone,
        F: for<'c> FnMut(&'c E::Key, &'c mut Cow<E>),
        <E::Tbl as Asset>::Trx<'a, 'b>: Insert<E>,
        for<'c> &'c <E::Tbl as Asset>::Trx<'a, 'b>: IntoIterator<Item = (&'c E::Key, &'c E)>,
    {
        self.asset::<E::Tbl>()
            .await?
            .update_with(updater, track)
            .await
    }

    #[inline]
    pub async fn update_mut_with<'b, E, F>(
        &'b mut self,
        updater: F,
        track: &E::TrackCtx,
    ) -> Result<()>
    where
        E: EntityAsset + PartialEq + ToOwned<Owned = E>,
        E::Key: Clone,
        F: for<'c> FnMut(&'c E::Key, &'c mut Cow<E>),
        <E::Tbl as Asset>::Trx<'a, 'b>: InsertMut<E>,
        for<'c> &'c <E::Tbl as Asset>::Trx<'a, 'b>: IntoIterator<Item = (&'c E::Key, &'c E)>,
    {
        self.asset::<E::Tbl>()
            .await?
            .update_mut_with(updater, track)
            .await
    }
}
