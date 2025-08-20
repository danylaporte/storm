use crate::{
    on_commit::call_on_commit,
    perform_apply_log,
    provider::{Delete, LoadAll, LoadArgs, LoadOne, TransactionProvider, Upsert, UpsertMut},
    registry::{perform_registration, provide_date},
    ApplyLog, AsRefAsync, AsyncTryFrom, BoxFuture, CtxExtObj, Entity, EntityAccessor, EntityRemove,
    EntityUpsert, EntityUpsertMut, Get, HashTable, Logs, ProviderContainer, RefIntoIterator,
    Result, Tag, Transaction, TrxErrGate, VecTable,
};
use chrono::NaiveDateTime;
use std::{borrow::Cow, hash::Hash};
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
    provider: TransactionProvider<'a>,
    pub ctx: &'a Ctx,
}

impl<'a> CtxTransaction<'a> {
    pub fn commit(mut self) -> BoxFuture<'a, Result<Logs>> {
        Box::pin(async move {
            self.err_gate.check()?;
            call_on_commit(&mut self).await?;
            self.provider.commit().await?;
            Ok(self.logs)
        })
    }

    #[inline]
    pub fn date(&self) -> NaiveDateTime {
        self.date
    }

    #[inline]
    pub fn provider(&self) -> &TransactionProvider<'a> {
        &self.provider
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
    pub fn insert_all_mut<'b, E, I>(&'b mut self, entities: I) -> BoxFuture<'b, Result<usize>>
    where
        E: EntityUpsertMut,
        I: IntoIterator<Item = (E::Key, E)>,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        for<'c> TransactionProvider<'c>: UpsertMut<E>,
    {
        let vec = entities.into_iter().collect();
        E::upsert_all_mut(self, vec)
    }

    #[inline]
    pub fn insert_mut<'b, E>(
        &'b mut self,
        key: &'b mut E::Key,
        entity: E,
    ) -> BoxFuture<'b, Result<bool>>
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
        E: Entity + EntityAccessor,
        Ctx: AsRefAsync<E::Tbl>,
    {
        Box::pin(async move {
            let tbl = self.ctx.tbl_of::<E>().await?;
            Ok(TblTransaction { tbl, ctx: self })
        })
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

pub struct TblTransaction<'a, 'b, E: Entity + EntityAccessor> {
    pub(crate) ctx: &'b mut CtxTransaction<'a>,
    tbl: &'a E::Tbl,
}

impl<'a, 'b, E> TblTransaction<'a, 'b, E>
where
    E: Entity + EntityAccessor,
    E::Key: Eq + Hash,
{
    #[inline]
    pub fn contains(&self, k: &E::Key) -> bool {
        self.get(k).is_some()
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
        ProviderContainer: LoadAll<E, (), E::Tbl> + Upsert<E>,
        for<'c> TransactionProvider<'c>: Upsert<E>,
    {
        self.ctx.insert_all::<E, _>(entities)
    }

    #[inline]
    pub fn insert_all_mut<I>(&mut self, entities: I) -> BoxFuture<'_, Result<usize>>
    where
        E: EntityUpsertMut,
        I: IntoIterator<Item = (E::Key, E)>,
        ProviderContainer: LoadAll<E, (), E::Tbl> + Upsert<E>,
        for<'c> TransactionProvider<'c>: UpsertMut<E>,
    {
        self.ctx.insert_all_mut::<E, _>(entities)
    }

    #[inline]
    pub fn insert_mut<'c>(
        &'c mut self,
        key: &'c mut E::Key,
        entity: E,
    ) -> BoxFuture<'c, Result<bool>>
    where
        E: EntityUpsertMut,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        for<'d> TransactionProvider<'d>: UpsertMut<E>,
    {
        E::upsert_mut(self.ctx, key, entity)
    }

    pub fn into_ref(self, k: &E::Key) -> Option<&'b E>
    where
        E: Entity + EntityAccessor,
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

impl ApplyLog<Logs> for async_cell_lock::QueueRwLockWriteGuard<'_, Ctx> {
    #[inline]
    fn apply_log(&mut self, logs: Logs) -> bool {
        perform_apply_log(&mut *self, logs)
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
            err_gate: Default::default(),
            logs: Default::default(),
            provider: self.provider.transaction(),
            user_id: user_id.into(),
        }
    }
}
