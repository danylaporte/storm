use parking_lot::RwLock;
use std::{hash::Hash, marker::PhantomData};

use crate::{
    provider::{Delete, LoadAll, TransactionProvider, Upsert},
    Accessor, ApplyLog, AsRefAsync, BoxFuture, Entity, EntityAccessor, Get, HashTable, Log,
    LogAccessor, LogCtx, ProviderContainer, Result, State, Transaction, VarCtx, VecTable,
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
            log: E::log_var().get_or_init_mut(&mut self.log_ctx, Default::default),
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
            Some(State::Inserted(v)) => Some(v),
            Some(State::Removed) => None,
            None => self.coll.get(k),
        }
    }

    pub async fn insert(&mut self, k: E::Key, v: E) -> Result<()>
    where
        TransactionProvider<'a>: Upsert<E>,
    {
        self.provider.upsert(&k, &v).await?;
        self.log.insert(k, State::Inserted(v));
        Ok(())
    }

    pub async fn remove(&mut self, k: E::Key) -> Result<()>
    where
        TransactionProvider<'a>: Delete<E>,
    {
        self.provider.delete(&k).await?;
        self.log.insert(k, State::Removed);
        Ok(())
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

impl<'a> ApplyLog<LogCtx> for async_cell_lock::QueueRwLockWriteGuard<'a, Ctx> {
    fn apply_log(&mut self, mut log: LogCtx) {
        let appliers = LOG_APPLIERS.read();

        for applier in &*appliers {
            applier.apply(&mut self.var_ctx, &mut log);
        }
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

    if let Some(v) = var.get(ctx) {
        return Box::pin(async move { Result::Ok(v) });
    }

    Box::pin(async move {
        let v = provider.load_all(&()).await?;
        Ok(var.get_or_init(ctx, || v))
    })
}

trait LogApplier: Send + Sync {
    fn apply(&self, var_ctx: &mut VarCtx, log_ctx: &mut LogCtx);
}

impl<E> LogApplier for EntityLogApplier<E>
where
    E: Entity + EntityAccessor + LogAccessor,
    E::Coll: ApplyLog<Log<E>>,
{
    fn apply(&self, var_ctx: &mut VarCtx, log_ctx: &mut LogCtx) {
        if let Some(log) = E::log_var().take(log_ctx) {
            if let Some(tbl) = E::entity_var().get_mut(var_ctx) {
                tbl.apply_log(log)
            }
        }
    }
}

struct EntityLogApplier<E: Entity + EntityAccessor + LogAccessor>(PhantomData<E>);

pub fn register_apply_log<E>()
where
    E: Entity + EntityAccessor + LogAccessor,
    E::Coll: ApplyLog<Log<E>>,
{
    LOG_APPLIERS
        .write()
        .push(Box::new(EntityLogApplier::<E>(PhantomData)));
}

#[static_init::dynamic]
static LOG_APPLIERS: RwLock<Vec<Box<dyn LogApplier>>> = Default::default();
