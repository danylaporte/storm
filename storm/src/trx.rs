use crate::{
    log::LogToken, provider::TransactionProvider, Asset, AssetBase, BoxFuture, Ctx, EntityAsset,
    GetOwned, Insert, InsertMut, Log, Remove, Result, TrxErrGate,
};
use std::{borrow::Cow, mem::transmute};

pub struct Trx<'a> {
    pub ctx: &'a Ctx,
    pub(crate) provider: TransactionProvider<'a>,
    pub log: Log,
    pub(crate) err_gate: TrxErrGate,
}

impl<'a> Trx<'a> {
    pub fn asset<'b, A: Asset>(&'b mut self) -> BoxFuture<'b, Result<A::Trx<'b>>> {
        Box::pin(async move {
            let asset = self.ctx.asset::<A>().await?;
            let this: &'b mut Trx<'b> = unsafe { transmute(self) };
            Ok(asset.trx(this, LogToken::from_asset::<A>()))
        })
    }

    pub fn asset_opt<'b, A: Asset>(&'b mut self) -> Option<A::Trx<'b>> {
        let asset = self.ctx.asset_opt::<A>()?;
        let this: &'b mut Trx<'b> = unsafe { transmute(self) };
        Some(asset.trx(this, LogToken::from_asset::<A>()))
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
        <E::Tbl as AssetBase>::Trx<'b>: GetOwned<'b, E, Q>,
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
        E: EntityAsset + PartialEq,
        for<'c> <E::Tbl as AssetBase>::Trx<'c>: Insert<E>,
    {
        Box::pin(async move {
            self.asset::<E::Tbl>()
                .await?
                .insert(id, entity, track)
                .await
        })
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
        for<'c> <E::Tbl as AssetBase>::Trx<'c>: InsertMut<E>,
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
        for<'c> <E::Tbl as AssetBase>::Trx<'c>: Remove<E>,
    {
        self.asset::<E::Tbl>().await?.remove(id, track).await
    }

    #[inline]
    pub fn remove_filter<'b, E, F>(
        &'b mut self,
        filter: F,
        track: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<()>>
    where
        for<'c> <E::Tbl as AssetBase>::Trx<'c>: Remove<E>,
        E: EntityAsset,
        E::Key: Clone,
        F: FnMut(&E::Key, &E) -> bool + Send + 'b,
        for<'c, 'd> &'d <E::Tbl as AssetBase>::Trx<'c>: IntoIterator<Item = (&'d E::Key, &'d E)>,
    {
        Box::pin(async move {
            self.asset::<E::Tbl>()
                .await?
                .remove_filter(filter, track)
                .await
        })
    }

    #[inline]
    pub fn tbl_of<'b, E: EntityAsset>(
        &'b mut self,
    ) -> BoxFuture<'b, Result<<E::Tbl as AssetBase>::Trx<'b>>> {
        self.asset::<E::Tbl>()
    }

    #[inline]
    pub fn update_with<'b, E, F>(
        &'b mut self,
        updater: F,
        track: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<()>>
    where
        E: EntityAsset + PartialEq + ToOwned<Owned = E>,
        E::Key: Clone,
        F: for<'c> FnMut(&'c E::Key, &'c mut Cow<E>) + Send + 'b,
        for<'c, 'd> &'d <E::Tbl as AssetBase>::Trx<'c>: IntoIterator<Item = (&'d E::Key, &'d E)>,
        for<'c> <E::Tbl as AssetBase>::Trx<'c>: Insert<E>,
    {
        Box::pin(async move {
            let mut asset = self.asset::<E::Tbl>().await?;
            asset.update_with(updater, track).await
        })
    }

    #[inline]
    pub fn update_mut_with<'b, E, F>(
        &'b mut self,
        updater: F,
        track: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<()>>
    where
        E: EntityAsset + PartialEq + ToOwned<Owned = E>,
        E::Key: Clone,
        F: for<'c> FnMut(&'c E::Key, &'c mut Cow<E>) + Send + 'b,
        for<'c, 'd> &'d <E::Tbl as AssetBase>::Trx<'c>: IntoIterator<Item = (&'d E::Key, &'d E)>,
        for<'c> <E::Tbl as AssetBase>::Trx<'c>: InsertMut<E>,
    {
        Box::pin(async move {
            self.asset::<E::Tbl>()
                .await?
                .update_mut_with(updater, track)
                .await
        })
    }
}
