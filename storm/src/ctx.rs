use crate::{
    length::Length,
    provider::{Delete, LoadAll, LoadArgs, LoadOne, TransactionProvider, Upsert, UpsertMut},
    register_metrics, Accessor, ApplyLog, AsRefAsync, AsyncTryFrom, BoxFuture, CtxTypeInfo, Entity,
    EntityAccessor, Gc, GcCtx, Get, HashTable, Insert, InsertMut, Log, LogAccessor, LogState, Logs,
    LogsVar, NotifyTag, ProviderContainer, Remove, Result, Tag, Transaction, Vars, VecTable,
};
use fxhash::FxHashMap;
use parking_lot::RwLock;
use std::{hash::Hash, marker::PhantomData};
use tracing::instrument;
use version_tag::VersionTag;

pub struct Ctx {
    pub(crate) gc: GcCtx,
    pub(crate) provider: ProviderContainer,
    vars: Vars,
}

impl Ctx {
    pub fn new(provider: ProviderContainer) -> Self {
        register_metrics();
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

    #[doc(hidden)]
    #[instrument(level = "debug", fields(name = <I as CtxTypeInfo>::NAME, obj = crate::OBJ_INDEX), skip(self))]
    pub fn index_gc<I>(&mut self)
    where
        I: Accessor + CtxTypeInfo + Gc,
    {
        if let Some(idx) = self.vars.get_mut(I::var()) {
            idx.gc(&self.gc);
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

        match self.vars.get_mut(E::entity_var()) {
            Some(ret) => {
                ret.notify_tag();
                Some(ret)
            }
            None => None,
        }
    }

    #[doc(hidden)]
    #[instrument(level = "debug", fields(name = <E as CtxTypeInfo>::NAME, obj = crate::OBJ_TABLE), skip(self))]
    pub fn tbl_gc<E>(&mut self)
    where
        E: CtxTypeInfo + Entity + EntityAccessor,
        E::Tbl: Accessor + NotifyTag + Gc,
    {
        if let Some(tbl) = self.vars.get_mut(E::entity_var()) {
            tbl.gc(&self.gc);
        }
    }

    pub fn vars(&self) -> &Vars {
        &self.vars
    }
}

impl Default for Ctx {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<E> AsRefAsync<HashTable<E>> for Ctx
where
    E: CtxTypeInfo + Entity + EntityAccessor<Tbl = HashTable<E>>,
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
    E: CtxTypeInfo + Entity + EntityAccessor<Tbl = VecTable<E>>,
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

impl<'a, L> CtxLocks<'a, L> {
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
    fn load_all_with_args<'b>(&'b self, filter: &'b F, args: LoadArgs) -> BoxFuture<'b, Result<C>> {
        self.ctx.load_all_with_args(filter, args)
    }
}

impl<'a, E, L> LoadOne<E> for CtxLocks<'a, L>
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

pub struct CtxTransaction<'a> {
    log_ctx: LogsVar,
    provider: TransactionProvider<'a>,
    pub ctx: &'a Ctx,
}

impl<'a> CtxTransaction<'a> {
    #[instrument(level = "debug", skip(self), err)]
    pub fn commit(self) -> BoxFuture<'a, Result<Logs>> {
        Box::pin(async move {
            self.provider.commit().await?;
            Ok(Logs(self.log_ctx))
        })
    }

    #[inline]
    pub fn provider(&self) -> &TransactionProvider<'a> {
        &self.provider
    }

    #[instrument(level = "debug", skip(self, k, v), err)]
    pub fn insert<'b, E>(
        &'b mut self,
        k: E::Key,
        v: E,
        track: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<()>>
    where
        Ctx: AsRefAsync<E::Tbl>,
        E: Entity + EntityAccessor + LogAccessor,
        TblTransaction<'a, 'b, E>: Insert<E>,
        'a: 'b,
    {
        Box::pin(async move { self.tbl_of::<E>().await?.insert(k, v, track).await })
    }

    #[instrument(level = "debug", skip(self, iter), err)]
    pub fn insert_all<'b, E, I>(
        &'b mut self,
        iter: I,
        track: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<usize>>
    where
        Ctx: AsRefAsync<E::Tbl>,
        E: Entity + EntityAccessor + LogAccessor,
        I: IntoIterator<Item = (E::Key, E)> + Send + 'b,
        I::IntoIter: Send,
        TblTransaction<'a, 'b, E>: Insert<E>,
        'a: 'b,
    {
        Box::pin(async move { self.tbl_of::<E>().await?.insert_all(iter, track).await })
    }

    #[instrument(level = "debug", skip(self, k, v), err)]
    pub fn insert_mut<'b, E>(
        &'b mut self,
        k: E::Key,
        v: E,
        track: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<E::Key>>
    where
        Ctx: AsRefAsync<E::Tbl>,
        E: Entity + EntityAccessor + LogAccessor,
        TblTransaction<'a, 'b, E>: InsertMut<E>,
        'a: 'b,
    {
        Box::pin(async move { self.tbl_of::<E>().await?.insert_mut(k, v, track).await })
    }

    #[instrument(level = "debug", skip(self, iter,), err)]
    pub fn insert_mut_all<'b, E, I>(
        &'b mut self,
        iter: I,
        track: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<usize>>
    where
        Ctx: AsRefAsync<E::Tbl>,
        E: Entity + EntityAccessor + LogAccessor,
        I: IntoIterator<Item = (E::Key, E)> + Send + 'b,
        I::IntoIter: Send,
        TblTransaction<'a, 'b, E>: InsertMut<E>,
        'a: 'b,
    {
        Box::pin(async move { self.tbl_of::<E>().await?.insert_mut_all(iter, track).await })
    }

    #[inline]
    pub fn ref_as<T>(&self) -> BoxFuture<'_, Result<&'_ T>>
    where
        Self: AsRefAsync<T>,
    {
        self.as_ref_async()
    }

    #[instrument(level = "debug", skip(self, k), err)]
    pub fn remove<'b, E>(
        &'b mut self,
        k: E::Key,
        track: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<()>>
    where
        Ctx: AsRefAsync<E::Tbl>,
        E: Entity + EntityAccessor + LogAccessor,
        TblTransaction<'a, 'b, E>: Remove<E>,
        'a: 'b,
    {
        Box::pin(async move { self.tbl_of::<E>().await?.remove(k, track).await })
    }

    #[instrument(level = "debug", skip(self, iter), err)]
    pub fn remove_all<'b, E, I>(
        &'b mut self,
        iter: I,
        track: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<usize>>
    where
        Ctx: AsRefAsync<E::Tbl>,
        E: Entity + EntityAccessor + LogAccessor,
        I: IntoIterator<Item = E::Key> + Send + 'b,
        I::IntoIter: Send,
        TblTransaction<'a, 'b, E>: Remove<E>,
        'a: 'b,
    {
        Box::pin(async move { self.tbl_of::<E>().await?.remove_all(iter, track).await })
    }

    pub fn tbl_of<'b, E>(&'b mut self) -> BoxFuture<'b, Result<TblTransaction<'a, 'b, E>>>
    where
        E: Entity + EntityAccessor + LogAccessor,
        Ctx: AsRefAsync<E::Tbl>,
        'a: 'b,
    {
        Box::pin(async move {
            let tbl = self.ctx.tbl_of::<E>().await?;
            Ok(TblTransaction { tbl, ctx: self })
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

pub struct TblTransaction<'a, 'b, E: Entity + EntityAccessor> {
    ctx: &'b mut CtxTransaction<'a>,
    tbl: &'a E::Tbl,
}

impl<'a, 'b, E> TblTransaction<'a, 'b, E>
where
    E: Entity + EntityAccessor,
    E::Key: Eq + Hash,
    E::Tbl: Get<E>,
{
    pub fn get(&self, k: &E::Key) -> Option<&E>
    where
        Self: Get<E>,
    {
        Get::get(self, k)
    }

    pub fn insert<'c>(
        &'c mut self,
        k: E::Key,
        v: E,
        track: &'c E::TrackCtx,
    ) -> BoxFuture<'c, Result<()>>
    where
        Self: Insert<E>,
    {
        Insert::insert(self, k, v, track)
    }

    pub fn insert_all<'c, I>(
        &'c mut self,
        iter: I,
        track: &'c E::TrackCtx,
    ) -> BoxFuture<'c, Result<usize>>
    where
        I: IntoIterator<Item = (E::Key, E)> + Send + 'a,
        I::IntoIter: Send,
        Self: Insert<E>,
    {
        Insert::<E>::insert_all(self, iter, track)
    }

    pub fn insert_mut<'c>(
        &'c mut self,
        k: E::Key,
        v: E,
        track: &'c E::TrackCtx,
    ) -> BoxFuture<'c, Result<E::Key>>
    where
        Self: InsertMut<E>,
    {
        InsertMut::insert_mut(self, k, v, track)
    }

    pub fn insert_mut_all<'c, I>(
        &'c mut self,
        iter: I,
        track: &'c E::TrackCtx,
    ) -> BoxFuture<'c, Result<usize>>
    where
        I: IntoIterator<Item = (E::Key, E)> + Send + 'a,
        I::IntoIter: Send,
        Self: InsertMut<E>,
    {
        InsertMut::<E>::insert_mut_all(self, iter, track)
    }

    pub fn log(&self) -> Option<&FxHashMap<E::Key, LogState<E>>>
    where
        E: Entity + EntityAccessor + LogAccessor,
    {
        self.ctx.log_ctx.get(E::log_var())
    }

    pub fn remove<'c>(&'c mut self, k: E::Key, track: &'c E::TrackCtx) -> BoxFuture<'c, Result<()>>
    where
        Self: Remove<E>,
    {
        Remove::<E>::remove(self, k, track)
    }

    pub fn remove_all<'c, K>(
        &'c mut self,
        keys: K,
        track: &'c E::TrackCtx,
    ) -> BoxFuture<'_, Result<usize>>
    where
        Self: Remove<E>,
        K: IntoIterator<Item = E::Key> + Send + 'c,
        K::IntoIter: Send,
    {
        Remove::<E>::remove_all(self, keys, track)
    }

    pub fn tbl(&self) -> &'a E::Tbl {
        self.tbl
    }
}

impl<'a, 'b, E> Get<E> for TblTransaction<'a, 'b, E>
where
    E: Entity + EntityAccessor + LogAccessor,
    E::Key: Eq + Hash,
    E::Tbl: Get<E>,
{
    fn get(&self, k: &E::Key) -> Option<&E> {
        match self.ctx.log_ctx.get(E::log_var()).and_then(|l| l.get(k)) {
            Some(LogState::Inserted(v)) => Some(v),
            Some(LogState::Removed) => None,
            None => self.tbl.get(k),
        }
    }
}

impl<'a, 'b, E> Length for TblTransaction<'a, 'b, E>
where
    E: Entity + EntityAccessor + LogAccessor,
    E::Key: Eq + Hash,
    E::Tbl: Get<E> + Length,
{
    fn len(&self) -> usize {
        let mut count = self.tbl.len();
        let logs = self.ctx.log_ctx.get(E::log_var());
        if let Some(logs) = logs {
            logs.iter().for_each(|(id, log)| {
                let is_present = self.get(id).is_some();
                match log {
                    LogState::Inserted(_) => {
                        if !is_present {
                            count += 1;
                        }
                    }
                    LogState::Removed => {
                        if is_present {
                            count -= 1;
                        }
                    }
                }
            });
        }
        count
    }
}

impl<'a, 'b, E> Insert<E> for TblTransaction<'a, 'b, E>
where
    for<'c> TransactionProvider<'c>: Upsert<E>,
    E: Entity + EntityAccessor + LogAccessor,
    E::Key: Eq + Hash,
    E::Tbl: Get<E>,
{
    #[instrument(level = "debug", skip(self, k, v), err)]
    fn insert<'c>(
        &'c mut self,
        k: E::Key,
        v: E,
        track: &'c E::TrackCtx,
    ) -> BoxFuture<'c, Result<()>> {
        Box::pin(async move {
            self.ctx.provider.upsert(&k, &v).await?;

            // remove first because if the track change the entity, we want to keep only the latest version.
            log_mut::<E>(&mut self.ctx.log_ctx).remove(&k);

            // change tracking...
            let old = self.tbl.get(&k);
            let result = v.track_insert(&k, old, self.ctx, track).await;

            // if the value is present, this is because the tracker has changed the value.
            log_mut(&mut self.ctx.log_ctx)
                .entry(k)
                .or_insert(LogState::Inserted(v));

            result
        })
    }
}

impl<'a, 'b, E> InsertMut<E> for TblTransaction<'a, 'b, E>
where
    for<'c> TransactionProvider<'c>: UpsertMut<E>,
    E: Entity + EntityAccessor + LogAccessor,
    E::Key: Clone + Eq + Hash,
    E::Tbl: Get<E>,
{
    #[instrument(level = "debug", skip(self, k, v), err)]
    fn insert_mut<'c>(
        &'c mut self,
        mut k: E::Key,
        mut v: E,
        track: &'c E::TrackCtx,
    ) -> BoxFuture<'c, Result<E::Key>> {
        Box::pin(async move {
            self.ctx.provider.upsert_mut(&mut k, &mut v).await?;

            // remove first because if the track change the entity, we want to keep only the latest version.
            log_mut::<E>(&mut self.ctx.log_ctx).remove(&k);

            // change tracking...
            let old = self.tbl.get(&k);
            let result = v.track_insert(&k, old, self.ctx, track).await;

            // if the value is present, this is because the tracker has changed the value.
            log_mut(&mut self.ctx.log_ctx)
                .entry(k.clone())
                .or_insert(LogState::Inserted(v));

            result.map(|_| k)
        })
    }
}

impl<'a, 'b, E> Remove<E> for TblTransaction<'a, 'b, E>
where
    for<'c> TransactionProvider<'c>: Delete<E>,
    E: Entity + EntityAccessor + LogAccessor,
    E::Key: Eq + Hash,
    E::Tbl: Accessor + Get<E>,
{
    #[instrument(level = "debug", skip(self, k), err)]
    fn remove<'c>(&'c mut self, k: E::Key, track: &'c E::TrackCtx) -> BoxFuture<'c, Result<()>> {
        Box::pin(async move {
            self.ctx.provider.delete(&k).await?;
            log_mut::<E>(&mut self.ctx.log_ctx).remove(&k);

            let mut result = Ok(());

            if let Some(old) = self.tbl.get(&k) {
                result = old.track_remove(&k, self.ctx, track).await;
            }

            log_mut::<E>(&mut self.ctx.log_ctx)
                .entry(k)
                .or_insert(LogState::Removed);

            result
        })
    }
}

impl<'a> ApplyLog<Logs> for async_cell_lock::QueueRwLockWriteGuard<'a, Ctx> {
    fn apply_log(&mut self, mut log: Logs) -> bool {
        let appliers = LOG_APPLIERS.read();
        let mut changed = false;

        for applier in &*appliers {
            changed = applier.apply(&mut self.vars, &mut log.0) || changed;
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
    fn apply(&self, vars: &mut Vars, log_ctx: &mut LogsVar) -> bool;
}

impl<E> LogApplier for EntityLogApplier<E>
where
    E: Entity + EntityAccessor + LogAccessor,
    E::Tbl: Accessor + ApplyLog<Log<E>>,
{
    fn apply(&self, vars: &mut Vars, log_ctx: &mut LogsVar) -> bool {
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

fn log_mut<E: Entity + LogAccessor>(logs: &mut LogsVar) -> &mut Log<E> {
    logs.get_or_init_mut(E::log_var(), Default::default)
}

#[doc(hidden)]
pub fn register_apply_log<E>()
where
    E: Entity + EntityAccessor + LogAccessor,
    E::Tbl: Accessor + ApplyLog<Log<E>>,
{
    register_apply_log_dyn(Box::new(EntityLogApplier::<E>(PhantomData)));
}

fn register_apply_log_dyn(app: Box<dyn LogApplier>) {
    LOG_APPLIERS.write().push(app);
}

#[static_init::dynamic]
static LOG_APPLIERS: RwLock<Vec<Box<dyn LogApplier>>> = Default::default();
