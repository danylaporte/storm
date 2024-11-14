use crate::{
    provider::{LoadAll, LoadArgs, LoadOne},
    trx_err_gate::TrxErrGate,
    AsRefAsync, Asset, AssetGc, AssetProxy, AsyncTryFrom, BoxFuture, ClearAssetEvent, Entity,
    EntityAsset, Log, ProviderContainer, Result, Tag, Trx,
};
use attached::{container, Container};
use version_tag::VersionTag;

container!(pub CtxVars);
pub(crate) type Assets = Container<CtxVars>;

pub struct Ctx {
    pub(crate) assets: Assets,
    gc: AssetGc,
    pub(crate) provider: ProviderContainer,
}

impl Ctx {
    pub fn new(provider: ProviderContainer) -> Self {
        Self {
            assets: Default::default(),
            gc: Default::default(),
            provider,
        }
    }

    #[inline]
    pub fn apply_log(&mut self, log: Log) -> bool {
        log.apply(self)
    }

    #[inline]
    pub async fn asset<A: AssetProxy>(&self) -> Result<&A::Asset> {
        self.asset_impl1::<A::Asset>().await
    }

    async fn asset_impl1<A: Asset>(&self) -> Result<&A> {
        match self.assets.get(A::ctx_var()) {
            Some(v) => Ok(v),
            None => self.asset_impl2().await,
        }
    }

    async fn asset_impl2<A: Asset>(&self) -> Result<&A> {
        let var = A::ctx_var();
        let _guard = self.provider.gate().await;

        if let Some(o) = self.assets.get(var) {
            return Ok(o);
        }

        let value = A::init(self).await?;

        self.gc.register::<A>();

        Ok(self.assets.get_or_init_val(var, value).0)
    }

    #[inline]
    pub fn asset_opt<A: AssetProxy>(&self) -> Option<&A::Asset> {
        self.asset_opt_imp()
    }

    #[inline]
    fn asset_opt_imp<A: Asset>(&self) -> Option<&A> {
        self.assets.get(A::ctx_var())
    }

    #[inline]
    pub fn clear_asset<A: AssetProxy>(&mut self) {
        self.clear_asset_imp::<A::Asset>();
    }

    fn clear_asset_imp<A: Asset>(&mut self) {
        if self.assets.replace(A::ctx_var(), None).is_some() {
            Self::on_clear_asset_imp::<A>().call(self);
        }
    }

    pub fn clear_tbl_of<E>(&mut self)
    where
        E: EntityAsset,
    {
        self.clear_asset_imp::<E::Tbl>();
    }

    pub fn gc(&mut self) {
        #[cfg(feature = "telemetry")]
        crate::telemetry::inc_storm_gc();

        self.provider.gc();
        self.gc.collect(&mut self.assets);
    }

    #[inline]
    pub fn on_clear_asset<A: AssetProxy>() -> &'static ClearAssetEvent {
        Self::on_clear_asset_imp::<A::Asset>()
    }

    fn on_clear_asset_imp<A: Asset>() -> &'static ClearAssetEvent {
        #[static_init::dynamic]
        static EVENT: ClearAssetEvent = Default::default();
        &EVENT
    }

    #[inline]
    pub fn provider(&self) -> &ProviderContainer {
        &self.provider
    }

    #[inline]
    pub async fn tbl_of<E: EntityAsset>(&self) -> Result<&E::Tbl> {
        self.asset_impl1::<E::Tbl>().await
    }

    #[inline]
    pub fn tbl_of_opt<E: EntityAsset>(&self) -> Option<&E::Tbl> {
        self.asset_opt_imp::<E::Tbl>()
    }

    #[must_use]
    pub fn transaction(&self) -> Trx<'_> {
        Trx {
            ctx: self,
            err_gate: TrxErrGate::default(),
            log: Log::default(),
            provider: self.provider.transaction(),
        }
    }
}

impl Default for Ctx {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<A: Asset> AsRefAsync<A> for Ctx {
    fn as_ref_async(&self) -> BoxFuture<'_, Result<&'_ A>> {
        Box::pin(self.asset_impl1::<A>())
    }
}

impl From<ProviderContainer> for Ctx {
    fn from(provider: ProviderContainer) -> Self {
        Self::new(provider)
    }
}

impl<E, F, C> LoadAll<E, F, C> for Ctx
where
    E: Entity,
    C: Default + Extend<(E::Key, E)> + Send,
    F: Send + Sync,
    ProviderContainer: LoadAll<E, F, C>,
{
    #[inline]
    fn load_all_with_args<'a>(&'a self, filter: &'a F, args: LoadArgs) -> BoxFuture<'a, Result<C>> {
        self.provider.load_all_with_args(filter, args)
    }
}

impl<E: Entity> LoadOne<E> for Ctx
where
    E: Entity,
    ProviderContainer: LoadOne<E>,
{
    #[inline]
    fn load_one_with_args<'a>(
        &'a self,
        k: &'a E::Key,
        args: LoadArgs,
    ) -> BoxFuture<'a, Result<Option<E>>> {
        self.provider.load_one_with_args(k, args)
    }
}

pub struct CtxLocks<'a, L> {
    pub ctx: &'a Ctx,
    pub locks: L,
}

impl<L> CtxLocks<'_, L> {
    #[inline]
    pub fn ref_as<T>(&self) -> &T
    where
        Self: AsRef<T>,
    {
        self.as_ref()
    }

    #[inline]
    pub fn ref_as_async<T>(&self) -> BoxFuture<'_, Result<&'_ T>>
    where
        Self: AsRefAsync<T>,
    {
        self.as_ref_async()
    }
}

impl<L> Tag for CtxLocks<'_, L>
where
    L: Tag,
{
    #[inline]
    fn tag(&self) -> VersionTag {
        self.locks.tag()
    }
}

impl<'a, L> AsyncTryFrom<'a, &'a Ctx> for CtxLocks<'a, L>
where
    L: AsyncTryFrom<'a, &'a Ctx>,
{
    fn async_try_from(ctx: &'a Ctx) -> BoxFuture<'a, Result<Self>> {
        Box::pin(async move {
            Ok(CtxLocks {
                ctx,
                locks: L::async_try_from(ctx).await?,
            })
        })
    }
}

impl<A, L> AsRef<A> for CtxLocks<'_, L>
where
    L: AsRef<A>,
{
    #[inline]
    fn as_ref(&self) -> &A {
        self.locks.as_ref()
    }
}

impl<E, F, C, L> LoadAll<E, F, C> for CtxLocks<'_, L>
where
    E: Entity,
    C: Default + Extend<(E::Key, E)> + Send,
    F: Send + Sync,
    L: Send + Sync,
    ProviderContainer: LoadAll<E, F, C>,
{
    #[inline]
    fn load_all_with_args<'b>(&'b self, filter: &'b F, args: LoadArgs) -> BoxFuture<'b, Result<C>> {
        self.ctx.load_all_with_args(filter, args)
    }
}

impl<E, L> LoadOne<E> for CtxLocks<'_, L>
where
    E: Entity,
    L: Send + Sync,
    ProviderContainer: LoadOne<E>,
{
    #[inline]
    fn load_one_with_args<'b>(
        &'b self,
        k: &'b E::Key,
        args: LoadArgs,
    ) -> BoxFuture<'b, Result<Option<E>>> {
        self.ctx.load_one_with_args(k, args)
    }
}
