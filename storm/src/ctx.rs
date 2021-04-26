use parking_lot::RwLock;
use std::{hash::Hash, marker::PhantomData};

use crate::{
    provider::{Delete, LoadAll, LoadOne, TransactionProvider, Upsert},
    Accessor, ApplyLog, AsRefAsync, BoxFuture, Entity, EntityAccessor, Get, HashTable, Insert, Log,
    LogAccessor, LogCtx, LogState, NotifyTag, ProviderContainer, Remove, Result, Transaction,
    VarCtx, VecTable,
};

#[derive(Default)]
pub struct Ctx {
    provider: ProviderContainer,
    var_ctx: VarCtx,
}

impl Ctx {
    pub fn new(provider: ProviderContainer) -> Self {
        Ctx {
            provider,
            var_ctx: VarCtx::new(),
        }
    }

    pub fn clear_tbl_of<E>(&mut self)
    where
        E: Entity + EntityAccessor,
        E::Coll: Accessor,
    {
        <E::Coll as Accessor>::clear_deps(&mut self.var_ctx);
        self.var_ctx.clear(E::entity_var());
    }

    #[inline]
    pub fn provider(&self) -> &ProviderContainer {
        &self.provider
    }

    #[inline]
    pub fn ref_as<T>(&self) -> BoxFuture<'_, Result<&'_ T>>
    where
        Self: AsRefAsync<T>,
    {
        self.as_ref_async()
    }

    #[inline]
    pub fn tbl_of<E>(&self) -> BoxFuture<'_, Result<&'_ E::Coll>>
    where
        E: Entity + EntityAccessor,
        Self: AsRefAsync<E::Coll>,
    {
        self.as_ref_async()
    }

    pub fn tbl_of_opt<E>(&self) -> Option<&E::Coll>
    where
        E: Entity + EntityAccessor,
        E::Coll: Accessor,
    {
        self.var_ctx.get(<E::Coll as Accessor>::var())
    }

    pub fn tbl_of_mut<E>(&mut self) -> Option<&mut E::Coll>
    where
        E: Entity + EntityAccessor,
        E::Coll: Accessor + NotifyTag,
    {
        <E::Coll as Accessor>::clear_deps(&mut self.var_ctx);

        let mut ret = self.var_ctx.get_mut(E::entity_var());

        if let Some(ret) = ret.as_mut() {
            ret.notify_tag();
        }

        ret
    }

    pub fn var_ctx(&self) -> &VarCtx {
        &self.var_ctx
    }
}

impl<E> AsRefAsync<HashTable<E>> for Ctx
where
    E: Entity + EntityAccessor<Coll = HashTable<E>>,
    E::Key: Eq + Hash,
    ProviderContainer: LoadAll<E, (), E::Coll>,
{
    #[inline]
    fn as_ref_async(&self) -> BoxFuture<'_, Result<&'_ E::Coll>> {
        table_as_ref_async::<E, _, _>(&self.var_ctx, &self.provider)
    }
}

impl<E> AsRefAsync<VecTable<E>> for Ctx
where
    E: Entity + EntityAccessor<Coll = VecTable<E>>,
    E::Key: Into<usize>,
    ProviderContainer: LoadAll<E, (), E::Coll>,
{
    #[inline]
    fn as_ref_async(&self) -> BoxFuture<'_, Result<&'_ E::Coll>> {
        table_as_ref_async::<E, _, _>(&self.var_ctx, &self.provider)
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
    fn load_all<'a>(&'a self, filter: &'a F) -> BoxFuture<'a, Result<C>> {
        self.provider.load_all(filter)
    }
}

impl<E: Entity> LoadOne<E> for Ctx
where
    E: Entity,
    ProviderContainer: LoadOne<E>,
{
    #[inline]
    fn load_one<'a>(&'a self, k: &'a E::Key) -> BoxFuture<'a, Result<Option<E>>> {
        self.provider.load_one(k)
    }
}

pub struct CtxLocks<'a, L> {
    pub ctx: &'a Ctx,
    pub locks: L,
}

impl<'a, L> CtxLocks<'a, L> {
    #[inline]
    pub fn ref_as<T>(&self) -> &T
    where
        Self: AsRef<T>,
    {
        self.as_ref()
    }

    pub fn ref_as_async<'b, T>(&'b self) -> BoxFuture<'b, Result<&'b T>>
    where
        Self: AsRefAsync<T>,
    {
        self.as_ref_async()
    }
}

impl<'a, E: Entity, L> AsRef<HashTable<E>> for CtxLocks<'a, L>
where
    L: AsRef<HashTable<E>>,
{
    #[inline]
    fn as_ref(&self) -> &HashTable<E> {
        self.locks.as_ref()
    }
}

impl<'a, E: Entity, L> AsRef<VecTable<E>> for CtxLocks<'a, L>
where
    L: AsRef<VecTable<E>>,
{
    #[inline]
    fn as_ref(&self) -> &VecTable<E> {
        self.locks.as_ref()
    }
}

impl<'a, E, F, C, L> LoadAll<E, F, C> for CtxLocks<'a, L>
where
    E: Entity,
    C: Default + Extend<(E::Key, E)> + Send,
    F: Send + Sync,
    L: Send + Sync,
    ProviderContainer: LoadAll<E, F, C>,
{
    #[inline]
    fn load_all<'b>(&'b self, filter: &'b F) -> BoxFuture<'b, Result<C>> {
        self.ctx.load_all(filter)
    }
}

impl<'a, E, L> LoadOne<E> for CtxLocks<'a, L>
where
    E: Entity,
    L: Send + Sync,
    ProviderContainer: LoadOne<E>,
{
    #[inline]
    fn load_one<'b>(&'b self, k: &'b E::Key) -> BoxFuture<'b, Result<Option<E>>> {
        self.ctx.load_one(k)
    }
}

pub struct CtxTransaction<'a> {
    log_ctx: LogCtx,
    provider: TransactionProvider<'a>,
    ctx: &'a Ctx,
}

impl<'a> CtxTransaction<'a> {
    pub async fn commit(self) -> Result<LogCtx> {
        Ok(self.log_ctx)
    }

    pub fn provider(&self) -> &TransactionProvider<'a> {
        &self.provider
    }

    pub async fn tbl_of<E>(&mut self) -> Result<TblTransaction<'_, E>>
    where
        E: Entity + EntityAccessor + LogAccessor,
        Ctx: AsRefAsync<E::Coll>,
    {
        Ok(TblTransaction {
            coll: self.ctx.as_ref_async().await?,
            provider: &self.provider,
            log: self.log_ctx.get_or_init_mut(E::log_var(), Default::default),
        })
    }
}

impl<'a, T> AsRefAsync<T> for CtxTransaction<'a>
where
    Ctx: AsRefAsync<T>,
{
    #[inline]
    fn as_ref_async(&self) -> BoxFuture<'_, Result<&'_ T>> {
        self.ctx.as_ref_async()
    }
}

pub struct TblTransaction<'a, E: Entity + EntityAccessor> {
    coll: &'a E::Coll,
    log: &'a mut Log<E>,
    provider: &'a TransactionProvider<'a>,
}

impl<'a, E> TblTransaction<'a, E>
where
    E: Entity + EntityAccessor,
    E::Key: Eq + Hash,
    E::Coll: Get<E>,
{
    pub fn get(&self, k: &E::Key) -> Option<&E> {
        match self.log.get(k) {
            Some(LogState::Inserted(v)) => Some(v),
            Some(LogState::Removed) => None,
            None => self.coll.get(k),
        }
    }

    #[inline]
    pub fn insert(&mut self, k: E::Key, v: E) -> BoxFuture<'_, Result<()>>
    where
        Self: Insert<E>,
    {
        Insert::insert(self, k, v)
    }

    #[inline]
    pub fn remove(&mut self, k: E::Key) -> BoxFuture<'_, Result<()>>
    where
        Self: Remove<E>,
    {
        Remove::<E>::remove(self, k)
    }
}

impl<'a, E> Get<E> for TblTransaction<'a, E>
where
    E: Entity + EntityAccessor,
    E::Coll: Get<E>,
    E::Key: Eq + Hash,
{
    #[inline]
    fn get(&self, k: &E::Key) -> Option<&E> {
        self.get(k)
    }
}

impl<'a, E> Insert<E> for TblTransaction<'a, E>
where
    for<'b> TransactionProvider<'b>: Upsert<E>,
    E: Entity + EntityAccessor,
    E::Key: Eq + Hash,
{
    fn insert(&mut self, k: E::Key, v: E) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            self.provider.upsert(&k, &v).await?;
            self.log.insert(k, LogState::Inserted(v));
            Ok(())
        })
    }
}

impl<'a, E> Remove<E> for TblTransaction<'a, E>
where
    for<'b> TransactionProvider<'b>: Delete<E>,
    E: Entity + EntityAccessor,
    E::Key: Eq + Hash,
    E::Coll: Accessor,
{
    fn remove(&mut self, k: E::Key) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            self.provider.delete(&k).await?;
            self.log.insert(k, LogState::Removed);
            Ok(())
        })
    }
}

impl<'a> ApplyLog<LogCtx> for async_cell_lock::QueueRwLockWriteGuard<'a, Ctx> {
    fn apply_log(&mut self, mut log: LogCtx) -> bool {
        let appliers = LOG_APPLIERS.read();
        let mut changed = false;

        for applier in &*appliers {
            changed = applier.apply(&mut self.var_ctx, &mut log) || changed;
        }

        changed
    }
}

impl<'a> Transaction for async_cell_lock::QueueRwLockQueueGuard<'a, Ctx> {
    fn transaction(&self) -> CtxTransaction<'_> {
        CtxTransaction {
            ctx: self,
            log_ctx: Default::default(),
            provider: self.provider.transaction(),
        }
    }
}

fn table_as_ref_async<'a, E, T, P>(ctx: &'a VarCtx, provider: &'a P) -> BoxFuture<'a, Result<&'a T>>
where
    T: Accessor + Default + Extend<(E::Key, E)> + Send + Sync,
    E: Entity + EntityAccessor<Coll = T>,
    P: LoadAll<E, (), T>,
{
    let var = T::var();

    if let Some(v) = ctx.get(var) {
        return Box::pin(async move { Result::Ok(v) });
    }

    Box::pin(async move {
        let v = provider.load_all(&()).await?;
        Ok(ctx.get_or_init(var, || v))
    })
}

trait LogApplier: Send + Sync {
    fn apply(&self, var_ctx: &mut VarCtx, log_ctx: &mut LogCtx) -> bool;
}

impl<E> LogApplier for EntityLogApplier<E>
where
    E: Entity + EntityAccessor + LogAccessor,
    E::Coll: Accessor + ApplyLog<Log<E>>,
{
    fn apply(&self, var_ctx: &mut VarCtx, log_ctx: &mut LogCtx) -> bool {
        if let Some(log) = log_ctx.replace(E::log_var(), None) {
            if let Some(tbl) = var_ctx.get_mut(E::entity_var()) {
                if tbl.apply_log(log) {
                    <E::Coll as Accessor>::clear_deps(var_ctx);
                    return true;
                }
            }
        }

        false
    }
}

struct EntityLogApplier<E: Entity + EntityAccessor + LogAccessor>(PhantomData<E>);

pub fn register_apply_log<E>()
where
    E: Entity + EntityAccessor + LogAccessor,
    E::Coll: Accessor + ApplyLog<Log<E>>,
{
    LOG_APPLIERS
        .write()
        .push(Box::new(EntityLogApplier::<E>(PhantomData)));
}

#[static_init::dynamic]
static LOG_APPLIERS: RwLock<Vec<Box<dyn LogApplier>>> = Default::default();
