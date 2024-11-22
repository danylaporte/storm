use crate::{
    log::LogToken, provider::TransactionProvider, Asset, AssetBase, BoxFuture, Ctx, EntityAsset,
    GetOwned, Insert, InsertMut, Log, Remove, Result, TrxErrGate,
};
use std::borrow::Cow;

pub struct Trx<'a> {
    pub ctx: &'a Ctx,
    pub(crate) provider: TransactionProvider<'a>,
    pub log: Log,
    pub(crate) err_gate: TrxErrGate,
}

impl<'a> Trx<'a> {
    pub fn asset<'b, A: Asset>(&'b mut self) -> BoxFuture<'b, Result<A::Trx<'a, 'b>>> {
        Box::pin(async move {
            Ok(self
                .ctx
                .asset::<A>()
                .await?
                .trx(self, LogToken::from_asset::<A>()))
        })
    }

    pub fn asset_opt<'b, A: Asset>(&'b mut self) -> Option<A::Trx<'a, 'b>>
    where
        'a: 'b,
    {
        Some(
            self.ctx
                .asset_opt::<A>()?
                .trx(self, LogToken::from_asset::<A>()),
        )
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
        <E::Tbl as AssetBase>::Trx<'a, 'b>: GetOwned<'b, E, Q>,
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
        for<'c, 'd> <E::Tbl as AssetBase>::Trx<'c, 'd>: Insert<E>,
        E: EntityAsset + PartialEq,
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
        <E::Tbl as AssetBase>::Trx<'a, 'b>: InsertMut<E>,
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
        <E::Tbl as AssetBase>::Trx<'a, 'b>: Remove<E>,
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
        for<'c, 'd> <E::Tbl as AssetBase>::Trx<'c, 'd>: Remove<E>,
        E: EntityAsset,
        E::Key: Clone,
        F: FnMut(&E::Key, &E) -> bool + Send + 'b,
        for<'c, 'd, 'e> &'c <E::Tbl as AssetBase>::Trx<'d, 'e>:
            IntoIterator<Item = (&'c E::Key, &'c E)>,
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
    ) -> BoxFuture<'b, Result<<E::Tbl as AssetBase>::Trx<'a, 'b>>>
    where
        'a: 'b,
    {
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
        for<'c, 'd, 'e> &'c <E::Tbl as AssetBase>::Trx<'d, 'e>:
            IntoIterator<Item = (&'c E::Key, &'c E)>,
        for<'c, 'd> <E::Tbl as AssetBase>::Trx<'c, 'd>: Insert<E>,
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
        for<'c, 'd, 'e> &'c <E::Tbl as AssetBase>::Trx<'d, 'e>:
            IntoIterator<Item = (&'c E::Key, &'c E)>,
        for<'c, 'd> <E::Tbl as AssetBase>::Trx<'c, 'd>: InsertMut<E>,
    {
        Box::pin(async move {
            self.asset::<E::Tbl>()
                .await?
                .update_mut_with(updater, track)
                .await
        })
    }
}
