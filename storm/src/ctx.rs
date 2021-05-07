use parking_lot::RwLock;
use std::{hash::Hash, marker::PhantomData};
use version_tag::VersionTag;

use crate::{
    provider::{Delete, LoadAll, LoadOne, TransactionProvider, Upsert},
    Accessor, ApplyLog, AsRefAsync, AsyncTryFrom, BoxFuture, Entity, EntityAccessor, GcCtx, Get,
    HashTable, Insert, Log, LogAccessor, LogState, Logs, NotifyTag, ProviderContainer, Remove,
    Result, Tag, Transaction, Vars, VecTable,
};

#[derive(Default)]
pub struct Ctx {
    pub(crate) gc: GcCtx,
    pub(crate) provider: ProviderContainer,
    vars: Vars,
}

impl Ctx {
    pub fn new(provider: ProviderContainer) -> Self {
        Ctx {
            gc: Default::default(),
            provider,
            vars: Vars::new(),
        }
    }

    pub fn clear_tbl_of<E>(&mut self)
    where
        E: Entity + EntityAccessor,
        E::Tbl: Accessor,
    {
        <E::Tbl as Accessor>::clear_deps(&mut self.vars);
        self.vars.clear(E::entity_var());
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
    pub fn tbl_of<E>(&self) -> BoxFuture<'_, Result<&'_ E::Tbl>>
    where
        E: Entity + EntityAccessor,
        Self: AsRefAsync<E::Tbl>,
    {
        self.as_ref_async()
    }

    pub fn tbl_of_opt<E>(&self) -> Option<&E::Tbl>
    where
        E: Entity + EntityAccessor,
        E::Tbl: Accessor,
    {
        self.vars.get(<E::Tbl as Accessor>::var())
    }

    pub fn tbl_of_mut<E>(&mut self) -> Option<&mut E::Tbl>
    where
        E: Entity + EntityAccessor,
        E::Tbl: Accessor + NotifyTag,
    {
        <E::Tbl as Accessor>::clear_deps(&mut self.vars);

        let mut ret = self.vars.get_mut(E::entity_var());

        if let Some(ret) = ret.as_mut() {
            ret.notify_tag();
        }

        ret
    }

    pub fn vars(&self) -> &Vars {
        &self.vars
    }

    pub fn vars_mut(&mut self) -> &Vars {
        &mut self.vars
    }
}

impl<E> AsRefAsync<HashTable<E>> for Ctx
where
    E: Entity + EntityAccessor<Tbl = HashTable<E>>,
    E::Key: Eq + Hash,
    ProviderContainer: LoadAll<E, (), E::Tbl>,
{
    #[inline]
    fn as_ref_async(&self) -> BoxFuture<'_, Result<&'_ E::Tbl>> {
        table_as_ref_async::<E, _>(&self.vars, &self.provider)
    }
}

impl<E> AsRefAsync<VecTable<E>> for Ctx
where
    E: Entity + EntityAccessor<Tbl = VecTable<E>>,
    E::Key: Into<usize>,
    ProviderContainer: LoadAll<E, (), E::Tbl>,
{
    #[inline]
    fn as_ref_async(&self) -> BoxFuture<'_, Result<&'_ E::Tbl>> {
        table_as_ref_async::<E, _>(&self.vars, &self.provider)
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

    pub fn ref_as_async<T>(&self) -> BoxFuture<'_, Result<&'_ T>>
    where
        Self: AsRefAsync<T>,
    {
        self.as_ref_async()
    }
}

impl<'a, L> Tag for CtxLocks<'a, L>
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
    #[inline]
    fn async_try_from(ctx: &'a Ctx) -> BoxFuture<'a, Result<Self>> {
        Box::pin(async move {
            Ok(CtxLocks {
                ctx,
                locks: L::async_try_from(ctx).await?,
            })
        })
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
    log_ctx: Logs,
    provider: TransactionProvider<'a>,
    pub ctx: &'a Ctx,
}

impl<'a> CtxTransaction<'a> {
    pub fn commit(self) -> BoxFuture<'a, Result<Logs>> {
        Box::pin(async move {
            self.provider.commit().await?;
            Ok(self.log_ctx)
        })
    }

    #[inline]
    pub fn provider(&self) -> &TransactionProvider<'a> {
        &self.provider
    }

    pub async fn insert_all<E, I>(&mut self, iter: I) -> Result<()>
    where
        Ctx: AsRefAsync<E::Tbl>,
        E: Entity + EntityAccessor + LogAccessor,
        I: IntoIterator<Item = (E::Key, E)> + Send,
        I::IntoIter: Send,
        for<'b> TblTransaction<'b, E>: Insert<E>,
    {
        self.tbl_of::<E>().await?.insert_all(iter).await?;
        Ok(())
    }

    #[inline]
    pub fn ref_as<T>(&self) -> BoxFuture<'_, Result<&'_ T>>
    where
        Self: AsRefAsync<T>,
    {
        self.as_ref_async()
    }

    pub async fn remove_all<E, I>(&mut self, iter: I) -> Result<()>
    where
        Ctx: AsRefAsync<E::Tbl>,
        E: Entity + EntityAccessor + LogAccessor,
        I: IntoIterator<Item = E::Key> + Send,
        I::IntoIter: Send,
        for<'b> TblTransaction<'b, E>: Remove<E>,
    {
        self.tbl_of::<E>().await?.remove_all(iter).await?;
        Ok(())
    }

    pub async fn tbl_of<E>(&mut self) -> Result<TblTransaction<'_, E>>
    where
        E: Entity + EntityAccessor + LogAccessor,
        Ctx: AsRefAsync<E::Tbl>,
    {
        Ok(TblTransaction {
            provider: &self.provider,
            log: self.log_ctx.get_or_init_mut(E::log_var(), Default::default),
            tbl: self.ctx.as_ref_async().await?,
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
    log: &'a mut Log<E>,
    provider: &'a TransactionProvider<'a>,
    tbl: &'a E::Tbl,
}

impl<'a, E> TblTransaction<'a, E>
where
    E: Entity + EntityAccessor,
    E::Key: Eq + Hash,
    E::Tbl: Get<E>,
{
    pub fn get(&self, k: &E::Key) -> Option<&E> {
        match self.log.get(k) {
            Some(LogState::Inserted(v)) => Some(v),
            Some(LogState::Removed) => None,
            None => self.tbl.get(k),
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
    pub fn insert_all<'b, I>(&'b mut self, iter: I) -> BoxFuture<'b, Result<()>>
    where
        I: IntoIterator<Item = (E::Key, E)> + Send + 'a,
        I::IntoIter: Send,
        Self: Insert<E>,
    {
        Insert::<E>::insert_all(self, iter)
    }

    #[inline]
    pub fn remove(&mut self, k: E::Key) -> BoxFuture<'_, Result<()>>
    where
        Self: Remove<E>,
    {
        Remove::<E>::remove(self, k)
    }

    #[inline]
    pub fn remove_all<'b, K>(&'b mut self, keys: K) -> BoxFuture<'_, Result<()>>
    where
        Self: Remove<E>,
        K: IntoIterator<Item = E::Key> + Send + 'b,
        K::IntoIter: Send,
    {
        Remove::<E>::remove_all(self, keys)
    }

    pub fn tbl(&self) -> &'a E::Tbl {
        self.tbl
    }
}

impl<'a, E> Get<E> for TblTransaction<'a, E>
where
    E: Entity + EntityAccessor,
    E::Key: Eq + Hash,
    E::Tbl: Get<E>,
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
    E::Tbl: Accessor,
{
    fn remove(&mut self, k: E::Key) -> BoxFuture<'_, Result<()>> {
        Box::pin(async move {
            self.provider.delete(&k).await?;
            self.log.insert(k, LogState::Removed);
            Ok(())
        })
    }
}

impl<'a> ApplyLog<Logs> for async_cell_lock::QueueRwLockWriteGuard<'a, Ctx> {
    fn apply_log(&mut self, mut log: Logs) -> bool {
        let appliers = LOG_APPLIERS.read();
        let mut changed = false;

        for applier in &*appliers {
            changed = applier.apply(&mut self.vars, &mut log) || changed;
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

fn table_as_ref_async<'a, E, T>(
    ctx: &'a Vars,
    provider: &'a ProviderContainer,
) -> BoxFuture<'a, Result<&'a T>>
where
    T: Accessor + Default + Extend<(E::Key, E)> + Send + Sync,
    E: Entity + EntityAccessor<Tbl = T>,
    ProviderContainer: LoadAll<E, (), T>,
{
    Box::pin(async move {
        let var = T::var();

        // get the table if already initialized.
        if let Some(v) = ctx.get(var) {
            return Ok(v);
        }

        // lock the provider to load the table.
        let _gate = provider.gate().await;

        // if the table is already loaded when we gain access to the provider.
        if let Some(v) = ctx.get(var) {
            return Ok(v);
        }

        // load the table
        let v = provider.load_all(&()).await?;
        Ok(ctx.get_or_init(var, || v))
    })
}

trait LogApplier: Send + Sync {
    fn apply(&self, vars: &mut Vars, log_ctx: &mut Logs) -> bool;
}

impl<E> LogApplier for EntityLogApplier<E>
where
    E: Entity + EntityAccessor + LogAccessor,
    E::Tbl: Accessor + ApplyLog<Log<E>>,
{
    fn apply(&self, vars: &mut Vars, log_ctx: &mut Logs) -> bool {
        if let Some(log) = log_ctx.replace(E::log_var(), None) {
            if let Some(tbl) = vars.get_mut(E::entity_var()) {
                if tbl.apply_log(log) {
                    <E::Tbl as Accessor>::clear_deps(vars);
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
    E::Tbl: Accessor + ApplyLog<Log<E>>,
{
    LOG_APPLIERS
        .write()
        .push(Box::new(EntityLogApplier::<E>(PhantomData)));
}

#[static_init::dynamic]
static LOG_APPLIERS: RwLock<Vec<Box<dyn LogApplier>>> = Default::default();
