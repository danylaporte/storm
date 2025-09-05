use crate::{
    indexing::AsyncAsIdxTrx,
    perform_apply_log,
    provider::{Delete, LoadAll, LoadArgs, LoadOne, TransactionProvider, Upsert, UpsertMut},
    registry::{perform_registration, provide_date},
    trx_iter::TblChangedIter,
    ApplyLog, AsRefAsync, AsyncTryFrom, BoxFuture, CommitEvent, CtxExtObj, Entity, EntityAccessor,
    EntityRemove, EntityUpsert, EntityUpsertMut, EventDepth, Get, HashTable, Logs,
    ProviderContainer, RefIntoIterator, Result, Tag, Transaction, TrxErrGate, VecTable,
};
use chrono::NaiveDateTime;
use fxhash::FxHashMap;
use std::{borrow::Cow, collections::hash_map, hash::Hash};
use uuid::Uuid;
use version_tag::VersionTag;

pub struct Ctx {
    pub(crate) provider: ProviderContainer,
    pub(crate) ctx_ext_obj: CtxExtObj,
}

impl Ctx {
    pub fn new(provider: ProviderContainer) -> Self {
        perform_registration();

        Ctx {
            provider,
            ctx_ext_obj: CtxExtObj::new(),
        }
    }

    #[inline]
    pub fn clear_tbl_of<E: EntityAccessor>(&mut self) {
        E::clear(self);
    }

    /// Private. Used in macros.
    #[doc(hidden)]
    #[inline]
    pub fn ctx_ext_obj(&self) -> &CtxExtObj {
        &self.ctx_ext_obj
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
        E: EntityAccessor,
        Self: AsRefAsync<E::Tbl>,
    {
        self.as_ref_async()
    }

    #[inline]
    pub fn tbl_of_opt<E>(&self) -> Option<&E::Tbl>
    where
        E: EntityAccessor,
    {
        self.ctx_ext_obj.get(E::tbl_var()).get()
    }
}

impl Default for Ctx {
    #[inline]
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl From<ProviderContainer> for Ctx {
    #[inline]
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

impl<E: Entity, L> AsRef<HashTable<E>> for CtxLocks<'_, L>
where
    L: AsRef<HashTable<E>>,
{
    #[inline]
    fn as_ref(&self) -> &HashTable<E> {
        self.locks.as_ref()
    }
}

impl<E: Entity, L> AsRef<VecTable<E>> for CtxLocks<'_, L>
where
    L: AsRef<VecTable<E>>,
{
    #[inline]
    fn as_ref(&self) -> &VecTable<E> {
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

pub struct CtxTransaction<'a> {
    pub(crate) date: NaiveDateTime,
    pub(crate) err_gate: TrxErrGate,
    pub(crate) logs: Logs,
    pub(crate) user_id: Uuid,
    depth: EventDepth,
    provider: TransactionProvider<'a>,
    pub ctx: &'a Ctx,
}

impl<'a> CtxTransaction<'a> {
    pub fn commit(mut self) -> BoxFuture<'a, Result<Logs>> {
        Box::pin(async move {
            self.err_gate.check()?;
            Self::commiting().call(&mut self).await?;
            self.provider.commit().await?;
            Ok(self.logs)
        })
    }

    #[inline]
    pub fn commiting() -> &'static CommitEvent {
        static EVENT: CommitEvent = CommitEvent::new();
        &EVENT
    }

    #[inline]
    pub fn date(&self) -> NaiveDateTime {
        self.date
    }

    #[inline]
    pub fn depth(&self) -> usize {
        self.depth.val()
    }

    #[inline]
    pub fn set_date(&mut self, date: NaiveDateTime) {
        self.date = date;
    }

    pub(crate) fn track_depth(&self) -> EventDepth {
        self.depth.clone()
    }

    #[inline]
    pub fn provider(&self) -> &TransactionProvider<'a> {
        &self.provider
    }

    #[inline]
    pub async fn index_trx<A>(&mut self) -> BoxFuture<'_, Result<A::Trx<'_>>>
    where
        A: AsyncAsIdxTrx,
    {
        A::async_as_idx_trx(self)
    }

    pub async fn get_entity<'b, E>(&'b mut self, k: &E::Key) -> Result<Option<&'b E>>
    where
        E: EntityAccessor,
        E::Key: Eq + Hash,
        Ctx: AsRefAsync<E::Tbl>,
    {
        self.tbl_of::<E>().await.map(|t| t.get_owned(k))
    }

    #[inline]
    pub fn insert<'b, E>(&'b mut self, key: E::Key, entity: E) -> BoxFuture<'b, Result<bool>>
    where
        E: EntityUpsert,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        for<'c> TransactionProvider<'c>: Upsert<E>,
    {
        E::upsert(self, key, entity)
    }

    #[inline]
    pub fn insert_all<'b, E, I>(&'b mut self, entities: I) -> BoxFuture<'b, Result<usize>>
    where
        E: EntityUpsert,
        I: IntoIterator<Item = (E::Key, E)>,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        for<'c> TransactionProvider<'c>: Upsert<E>,
    {
        let vec = entities.into_iter().collect();
        E::upsert_all(self, vec)
    }

    #[inline]
    pub fn insert_mut_all<'b, E, I>(&'b mut self, entities: I) -> BoxFuture<'b, Result<usize>>
    where
        E: EntityUpsertMut,
        I: IntoIterator<Item = (E::Key, E)>,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        for<'c> TransactionProvider<'c>: UpsertMut<E>,
    {
        let vec = entities.into_iter().collect();
        E::upsert_mut_all(self, vec)
    }

    #[inline]
    pub fn insert_mut<'b, E>(
        &'b mut self,
        key: E::Key,
        entity: E,
    ) -> BoxFuture<'b, Result<(E::Key, bool)>>
    where
        E: EntityUpsertMut,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        for<'c> TransactionProvider<'c>: UpsertMut<E>,
    {
        E::upsert_mut(self, key, entity)
    }

    #[inline]
    pub fn ref_as<T>(&self) -> BoxFuture<'_, Result<&'_ T>>
    where
        Self: AsRefAsync<T>,
    {
        self.as_ref_async()
    }

    #[inline]
    pub fn remove<'b, E>(&'b mut self, k: E::Key) -> BoxFuture<'b, Result<bool>>
    where
        E: EntityRemove,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        for<'c> TransactionProvider<'c>: Delete<E>,
    {
        E::remove(self, k)
    }

    #[inline]
    pub fn remove_all<'b, E, I>(&'b mut self, keys: I) -> BoxFuture<'b, Result<usize>>
    where
        E: EntityRemove,
        I: IntoIterator<Item = E::Key>,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        for<'c> TransactionProvider<'c>: Delete<E>,
    {
        let keys = keys.into_iter().collect::<Vec<_>>();
        E::remove_all(self, Cow::Owned(keys))
    }

    pub async fn remove_filter<'b, E, F>(&'b mut self, mut filter: F) -> Result<usize>
    where
        Ctx: AsRefAsync<E::Tbl>,
        E: EntityRemove,
        F: FnMut(&E::Key, &E) -> bool,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        for<'c> &'c E::Tbl: IntoIterator<Item = (&'c E::Key, &'c E)>,
        for<'c> TransactionProvider<'c>: Delete<E>,
    {
        let tbl = self.ctx.tbl_of::<E>().await?;

        let ids = tbl
            .into_iter()
            .filter(|t| filter(t.0, t.1))
            .map(|t| t.0.clone())
            .collect::<Vec<E::Key>>();

        E::remove_all(self, Cow::Owned(ids)).await
    }

    #[inline]
    pub fn tbl_of<'b, E>(&'b mut self) -> BoxFuture<'b, Result<TblTransaction<'a, 'b, E>>>
    where
        E: EntityAccessor,
        Ctx: AsRefAsync<E::Tbl>,
    {
        Box::pin(async move {
            let tbl = self.ctx.tbl_of::<E>().await?;
            Ok(TblTransaction { tbl, ctx: self })
        })
    }

    pub fn tbl_changes<E: EntityAccessor>(&self) -> TblChangedIter<'_, E> {
        TblChangedIter {
            log_iter: self.logs.get(E::tbl_var()).map(|h| h.iter()),
            tbl: self.ctx.ctx_ext_obj.get(E::tbl_var()).get(),
        }
    }

    /// Indicate if the table specified ty the entity E has been touched, inserted or removed.
    #[inline]
    pub fn tbl_touched<E>(&self) -> bool
    where
        E: EntityAccessor,
    {
        self.logs.contains(E::tbl_var())
    }

    pub async fn update_with<E, F>(&mut self, mut updater: F) -> Result<()>
    where
        E: EntityUpsert + ToOwned<Owned = E>,
        F: for<'c> FnMut(&'c E::Key, &'c mut Cow<E>) -> Result<()>,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        for<'c> TransactionProvider<'c>: Upsert<E>,
    {
        let vec = self
            .logs
            .get(E::tbl_var())
            .map(|l| l.iter())
            .into_iter()
            .flatten()
            .filter_map(|(id, state)| {
                if let Some(e) = state {
                    let mut e = Cow::Borrowed(e);

                    if let Err(err) = updater(id, &mut e) {
                        return Some(Err(err));
                    }

                    if let Cow::Owned(e) = e {
                        return Some(Ok((id.clone(), e)));
                    }
                }

                None
            })
            .collect::<Result<Vec<(E::Key, E)>>>()?;

        E::upsert_all(self, vec).await?;

        let tbl = E::tbl_from(self.ctx).await?;

        for (id, e) in tbl.ref_iter() {
            if self
                .logs
                .get(E::tbl_var())
                .is_none_or(|l| !l.contains_key(id))
            {
                let mut e = Cow::Borrowed(e);

                updater(id, &mut e)?;

                if let Cow::Owned(e) = e {
                    E::upsert(self, id.clone(), e).await?;
                }
            }
        }

        Ok(())
    }

    pub async fn update_mut_with<E, F>(&mut self, mut updater: F) -> Result<()>
    where
        E: EntityUpsertMut + ToOwned<Owned = E>,
        F: for<'c> FnMut(&'c E::Key, &'c mut Cow<E>) -> Result<()>,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        for<'c> TransactionProvider<'c>: UpsertMut<E>,
    {
        let vec = self
            .logs
            .get(E::tbl_var())
            .map(|l| l.iter())
            .into_iter()
            .flatten()
            .filter_map(|(id, state)| {
                if let Some(e) = state {
                    let mut e = Cow::Borrowed(e);

                    if let Err(err) = updater(id, &mut e) {
                        return Some(Err(err));
                    }

                    if let Cow::Owned(e) = e {
                        return Some(Ok((id.clone(), e)));
                    }
                }

                None
            })
            .collect::<Result<Vec<(E::Key, E)>>>()?;

        E::upsert_mut_all(self, vec).await?;

        let tbl = E::tbl_from(self.ctx).await?;

        for (id, e) in tbl.ref_iter() {
            if self
                .logs
                .get(E::tbl_var())
                .is_none_or(|l| !l.contains_key(id))
            {
                let mut e = Cow::Borrowed(e);

                updater(id, &mut e)?;

                if let Cow::Owned(e) = e {
                    E::upsert_mut(self, id.clone(), e).await?;
                }
            }
        }

        Ok(())
    }

    #[inline]
    pub fn user_id<U: From<Uuid>>(&self) -> U {
        self.user_id.into()
    }
}

impl<T> AsRefAsync<T> for CtxTransaction<'_>
where
    Ctx: AsRefAsync<T>,
{
    #[inline]
    fn as_ref_async(&self) -> BoxFuture<'_, Result<&'_ T>> {
        self.ctx.as_ref_async()
    }
}

pub struct TblTransaction<'a, 'b, E: EntityAccessor> {
    pub(crate) ctx: &'b mut CtxTransaction<'a>,
    tbl: &'a E::Tbl,
}

impl<'a, 'b, E> TblTransaction<'a, 'b, E>
where
    E: EntityAccessor,
    E::Key: Eq + Hash,
{
    #[inline]
    pub fn contains(&self, k: &E::Key) -> bool {
        self.get(k).is_some()
    }

    #[inline]
    pub fn date(&self) -> NaiveDateTime {
        self.ctx.date
    }

    /// gets a reference from the log or the underlying ctx.
    ///
    /// You can take the TblTransaction by ownership and have a longer
    /// lifetime for the & by using the [Self::into_ref] method.
    #[inline]
    pub fn get<'c>(&'c self, k: &E::Key) -> Option<&'c E> {
        Get::get(self, k)
    }

    /// Gets a reference by consuming the tbl transaction. This provide a longer reference.
    pub fn get_owned(self, k: &E::Key) -> Option<&'b E>
    where
        E: EntityAccessor,
        E::Tbl: Get<E>,
    {
        match self.ctx.logs.get(E::tbl_var()).and_then(|l| l.get(k)) {
            Some(Some(v)) => Some(v),
            Some(None) => None,
            None => self.tbl.get(k),
        }
    }

    #[inline]
    pub fn insert<'c>(&'c mut self, key: E::Key, entity: E) -> BoxFuture<'c, Result<bool>>
    where
        E: EntityUpsert,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        for<'d> TransactionProvider<'d>: Upsert<E>,
    {
        E::upsert(self.ctx, key, entity)
    }

    #[inline]
    pub fn insert_all<I>(&mut self, entities: I) -> BoxFuture<'_, Result<usize>>
    where
        E: EntityUpsert,
        I: IntoIterator<Item = (E::Key, E)>,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        for<'c> TransactionProvider<'c>: Upsert<E>,
    {
        self.ctx.insert_all::<E, _>(entities)
    }

    #[inline]
    pub fn insert_mut_all<I>(&mut self, entities: I) -> BoxFuture<'_, Result<usize>>
    where
        E: EntityUpsertMut,
        I: IntoIterator<Item = (E::Key, E)>,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        for<'c> TransactionProvider<'c>: UpsertMut<E>,
    {
        self.ctx.insert_mut_all::<E, _>(entities)
    }

    #[inline]
    pub fn insert_mut<'c>(
        &'c mut self,
        key: E::Key,
        entity: E,
    ) -> BoxFuture<'c, Result<(E::Key, bool)>>
    where
        E: EntityUpsertMut,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        for<'d> TransactionProvider<'d>: UpsertMut<E>,
    {
        E::upsert_mut(self.ctx, key, entity)
    }

    pub fn keys(&self) -> impl Iterator<Item = &E::Key> {
        let tbl = self.ctx.ctx.ctx_ext_obj.get(E::tbl_var()).get();
        let log = self.ctx.logs.get(E::tbl_var());

        tbl.zip(log).into_iter().flat_map(|(tbl, log)| {
            tbl.ref_iter()
                .map(|(k, _)| k)
                .filter(|k| !log.contains_key(k))
                .chain(log.keys())
        })
    }

    pub fn into_ref(self, k: &E::Key) -> Option<&'b E>
    where
        E: EntityAccessor,
        E::Key: Eq + Hash,
    {
        match self.ctx.logs.get(E::tbl_var()).and_then(|l| l.get(k)) {
            Some(Some(v)) => Some(v),
            Some(None) => None,
            None => self.tbl.get(k),
        }
    }

    #[inline]
    pub fn remove<'c>(&'c mut self, k: E::Key) -> BoxFuture<'c, Result<bool>>
    where
        E: EntityRemove,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        for<'d> TransactionProvider<'d>: Delete<E>,
    {
        E::remove(self.ctx, k)
    }

    #[inline]
    pub fn remove_all<I>(&mut self, keys: I) -> BoxFuture<'_, Result<usize>>
    where
        E: EntityRemove,
        I: IntoIterator<Item = E::Key>,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        for<'c> TransactionProvider<'c>: Delete<E>,
    {
        let keys = keys.into_iter().collect::<Vec<_>>();
        E::remove_all(self.ctx, Cow::Owned(keys))
    }

    #[inline]
    pub async fn remove_filter<F>(&mut self, filter: F) -> Result<usize>
    where
        Ctx: AsRefAsync<E::Tbl>,
        E: EntityRemove,
        F: FnMut(&E::Key, &E) -> bool,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        for<'c> &'c E::Tbl: IntoIterator<Item = (&'c E::Key, &'c E)> + Get<E>,
        for<'c> TransactionProvider<'c>: Delete<E>,
    {
        self.ctx.remove_filter::<E, F>(filter).await
    }

    #[inline]
    pub fn trx(&self) -> &CtxTransaction {
        self.ctx
    }

    #[inline]
    pub fn tbl(&self) -> &'a E::Tbl {
        self.tbl
    }

    #[inline]
    pub async fn update_with<F>(&mut self, updater: F) -> Result<()>
    where
        E: EntityUpsert + ToOwned<Owned = E>,
        F: for<'c> FnMut(&'c E::Key, &'c mut Cow<E>) -> Result<()>,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        for<'c> &'c E::Tbl: IntoIterator<Item = (&'c E::Key, &'c E)> + Get<E>,
        for<'c> TransactionProvider<'c>: Upsert<E>,
    {
        self.ctx.update_with::<E, F>(updater).await
    }

    #[inline]
    pub async fn update_mut_with<F>(&mut self, updater: F) -> Result<()>
    where
        E: EntityUpsertMut + ToOwned<Owned = E>,
        F: for<'c> FnMut(&'c E::Key, &'c mut Cow<E>) -> Result<()>,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        for<'c> &'c E::Tbl: IntoIterator<Item = (&'c E::Key, &'c E)> + Get<E>,
        for<'c> TransactionProvider<'c>: UpsertMut<E>,
    {
        self.ctx.update_mut_with::<E, F>(updater).await
    }

    #[inline]
    pub fn user_id<U: From<Uuid>>(&self) -> U {
        self.ctx.user_id()
    }
}

impl<'a, 'b, E: EntityAccessor> Get<E> for TblTransaction<'a, 'b, E> {
    fn get(&self, key: &E::Key) -> Option<&E> {
        match self.ctx.logs.get(E::tbl_var()).and_then(|l| l.get(key)) {
            Some(Some(v)) => Some(v),
            Some(None) => None,
            None => self.tbl.get(key),
        }
    }
}

pub struct TblTransactionIter<'a, E: EntityAccessor> {
    tbl_iter: <E::Tbl as RefIntoIterator>::Iter<'a>,
    log: Option<&'a FxHashMap<E::Key, Option<E>>>,
    log_iter: Option<hash_map::Iter<'a, E::Key, Option<E>>>,
}

impl<'a, E> Iterator for TblTransactionIter<'a, E>
where
    E: EntityAccessor,
{
    type Item = (&'a E::Key, &'a E);

    fn next(&mut self) -> Option<Self::Item> {
        for (id, e) in self.tbl_iter.by_ref() {
            if self.log.is_none_or(|log| !log.contains_key(id)) {
                return Some((id, e));
            }
        }

        if let Some(log_iter) = self.log_iter.as_mut() {
            for (id, e) in log_iter.by_ref() {
                if let Some(v) = e.as_ref() {
                    return Some((id, v));
                }
            }
        }

        None
    }
}

impl<'a, 'b, E: EntityAccessor> RefIntoIterator for TblTransaction<'a, 'b, E> {
    type Item<'c>
        = (&'c E::Key, &'c E)
    where
        Self: 'c;
    type Iter<'c>
        = TblTransactionIter<'c, E>
    where
        Self: 'c;

    fn ref_iter(&self) -> Self::Iter<'_> {
        let log = self.ctx.logs.get(E::tbl_var());
        TblTransactionIter {
            tbl_iter: self.tbl.ref_iter(),
            log_iter: log.map(|log| log.iter()),
            log,
        }
    }
}

impl ApplyLog<Logs> for async_cell_lock::QueueRwLockWriteGuard<'_, Ctx> {
    fn apply_log(&mut self, logs: Logs) -> bool {
        #[cfg(feature = "telemetry")]
        let instant = std::time::Instant::now();

        let changed = perform_apply_log(&mut *self, logs);

        #[cfg(feature = "telemetry")]
        {
            let elapsed = instant.elapsed().as_millis();

            if elapsed > 250 {
                tracing::warn!(elapsed_ms = elapsed, "apply_log took too long",);
            }
        }

        changed
    }
}

impl Transaction for async_cell_lock::QueueRwLockQueueGuard<'_, Ctx> {
    fn transaction<U>(&self, user_id: U) -> CtxTransaction<'_>
    where
        U: Into<Uuid>,
    {
        CtxTransaction {
            ctx: self,
            date: provide_date(),
            depth: Default::default(),
            err_gate: Default::default(),
            logs: Default::default(),
            provider: self.provider.transaction(),
            user_id: user_id.into(),
        }
    }
}
