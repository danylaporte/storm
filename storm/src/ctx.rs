use std::any::TypeId;

use crate::{
    cycle_dep,
    provider::{LoadAll, LoadArgs, LoadOne},
    trx_err_gate::TrxErrGate,
    AsRefAsync, AsyncTryFrom, BoxFuture, ClearEvent, Entity, EntityObj, Log, Obj, ObjGc,
    ProviderContainer, Result, Tag, Trx,
};
use attached::{container, Container};
use version_tag::VersionTag;

container!(pub CtxVars);
pub(crate) type Objs = Container<CtxVars>;

pub struct Ctx {
    pub(crate) objs: Objs,
    gc: ObjGc,
    pub(crate) provider: ProviderContainer,
}

impl Ctx {
    pub fn new(provider: ProviderContainer) -> Self {
        Self {
            objs: Default::default(),
            gc: Default::default(),
            provider,
        }
    }

    #[inline]
    pub fn apply_log(&mut self, log: Log) -> bool {
        log.apply(self)
    }

    pub fn clear<A: Obj>(&mut self) {
        if self.objs.take(A::ctx_var()).is_some() {
            Self::on_clear_obj::<A>().call(self);
        }
    }

    #[inline]
    pub fn clear_tbl_of<E>(&mut self)
    where
        E: EntityObj,
    {
        self.clear::<E::Tbl>();
    }

    pub fn obj<A: Obj>(&self) -> BoxFuture<'_, Result<&A>> {
        Box::pin(async move {
            let var = A::ctx_var();

            if let Some(v) = self.objs.get(var) {
                return Ok(v);
            }

            let id = TypeId::of::<A>();

            let (out, loaded) = cycle_dep::guard(
                |should_lock| async move {
                    let _guard = if should_lock {
                        Some(self.provider.gate().await)
                    } else {
                        None
                    };

                    if let Some(o) = self.objs.get(var) {
                        return Ok((o, false));
                    }

                    let value = A::init(self).await?;

                    self.gc.register::<A>();

                    Ok((self.objs.get_or_init(var, || value), true))
                },
                id,
            )
            .await?;

            if loaded {
                // To prevent cycle, we call the loaded event after loaded state.
                A::loaded().call(self).await?;
            }

            Ok(out)
        })
    }

    #[inline]
    pub fn obj_opt<A: Obj>(&self) -> Option<&A> {
        self.objs.get(A::ctx_var())
    }

    pub fn gc(&mut self) {
        #[cfg(feature = "telemetry")]
        crate::telemetry::inc_storm_gc();

        self.provider.gc();
        self.gc.collect(&mut self.objs);
    }

    #[inline]
    pub fn on_clear_obj<A: Obj>() -> &'static ClearEvent {
        #[static_init::dynamic]
        static EVENT: ClearEvent = Default::default();
        &EVENT
    }

    #[inline]
    pub fn provider(&self) -> &ProviderContainer {
        &self.provider
    }

    #[inline]
    pub async fn tbl_of<E: EntityObj>(&self) -> Result<&E::Tbl> {
        self.obj::<E::Tbl>().await
    }

    #[inline]
    pub fn tbl_of_opt<E: EntityObj>(&self) -> Option<&E::Tbl> {
        self.obj_opt::<E::Tbl>()
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

impl<A> AsRefAsync<A> for Ctx
where
    A: Obj + Sync,
{
    #[inline]
    fn as_ref_async(&self) -> BoxFuture<'_, Result<&'_ A>> {
        Box::pin(self.obj())
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
