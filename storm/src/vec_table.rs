use crate::{
    log::{LogToken, LogVars},
    provider::{Delete, LoadAll, LoadArgs, TransactionProvider, Upsert, UpsertMut},
    validate_on_change, Asset, AssetBase, BoxFuture, CtxTypeInfo, CtxVars, Entity, EntityAsset,
    EntityValidate, Get, GetMut, GetOwned, Insert, InsertMut, ProviderContainer, Remove, Result,
    Tag, Trx,
};
use attached::Var;
use fxhash::FxHashMap;
use rayon::iter::IntoParallelIterator;
use std::{borrow::Cow, future::Future, hash::Hash};
use vec_map::{Entry, Iter, Keys, ParIter, Values, VecMap};
use version_tag::VersionTag;

type Log<E> = FxHashMap<<E as Entity>::Key, Option<E>>;

pub struct VecTable<E: EntityAsset<Tbl = Self>> {
    map: VecMap<E::Key, E>,
    tag: VersionTag,
}

impl<E> VecTable<E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = Self>,
{
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn get<'a>(&'a self, key: &E::Key) -> Option<&'a E>
    where
        E::Key: Copy,
        usize: From<E::Key>,
    {
        self.map.get(key)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    #[inline]
    pub fn iter(&self) -> Iter<E::Key, E> {
        self.map.iter()
    }

    #[inline]
    pub fn keys(&self) -> Keys<E::Key, E> {
        self.map.keys()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    fn update_metrics(&self) {
        #[cfg(feature = "telemetry")]
        crate::telemetry::update_storm_table_rows(self.len(), E::NAME);
    }

    #[inline]
    pub fn values(&self) -> Values<E::Key, E> {
        self.map.values()
    }
}

impl<E> Asset for VecTable<E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = Self> + PartialEq,
    E::Key: Copy,
    ProviderContainer: LoadAll<E, (), Self>,
    usize: From<E::Key>,
{
    #[inline]
    fn ctx_var() -> Var<Self, CtxVars> {
        E::ctx_var()
    }

    fn init(ctx: &crate::Ctx) -> impl Future<Output = Result<Self>> {
        ctx.provider.load_all_with_args(&(), LoadArgs::default())
    }

    #[inline]
    fn log_var() -> Var<Self::Log, LogVars> {
        E::log_var()
    }
}

impl<E> AssetBase for VecTable<E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = Self> + PartialEq + 'static,
    E::Key: Copy,
    usize: From<E::Key>,
{
    const SUPPORT_GC: bool = E::SUPPORT_GC;

    type Log = Log<E>;
    type Trx<'a> = VecTableTrx<'a, E>;

    fn apply_log(&mut self, log: Self::Log) -> bool {
        let mut changed = false;

        for (key, o) in log {
            match o {
                Some(v) => match self.map.entry(key) {
                    Entry::Occupied(mut e) => {
                        changed |= *e.get() != v;
                        e.insert(v);
                    }
                    Entry::Vacant(e) => {
                        changed = true;
                        e.insert(v);
                    }
                },
                None => changed = self.map.remove(&key).is_some() || changed,
            };
        }

        if changed {
            self.update_metrics();
            self.tag.notify();
        }

        changed
    }

    fn gc(&mut self) {
        self.map.values_mut().for_each(|e| e.gc());
    }

    fn trx<'a>(&'a self, trx: &'a mut Trx<'a>, log: LogToken<Self::Log>) -> Self::Trx<'a> {
        VecTableTrx {
            log_token: log,
            tbl: self,
            trx,
        }
    }
}

impl<E> Clone for VecTable<E>
where
    E: Clone + EntityAsset<Tbl = Self>,
    E::Key: Clone,
{
    fn clone(&self) -> Self {
        Self {
            map: self.map.clone(),
            tag: self.tag,
        }
    }
}

impl<E: EntityAsset<Tbl = Self>> Default for VecTable<E> {
    #[inline]
    fn default() -> Self {
        Self {
            map: VecMap::new(),
            tag: VersionTag::new(),
        }
    }
}

impl<E> Extend<(E::Key, E)> for VecTable<E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = Self> + PartialEq + 'static,
    E::Key: Copy + Into<usize>,
    usize: From<E::Key>,
{
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = (E::Key, E)>,
    {
        for (k, v) in iter {
            self.map.insert(k, v);
        }

        self.update_metrics();
    }
}

impl<E, Q> Get<E, Q> for VecTable<E>
where
    E: CtxTypeInfo + EntityAsset<Key = Q, Tbl = VecTable<E>>,
    Q: Copy + 'static,
    usize: From<Q>,
{
    #[inline]
    fn get_entity<'a>(&'a self, q: &Q) -> Option<&'a E> {
        self.map.get(q)
    }
}

impl<E> GetMut<E> for VecTable<E>
where
    E: EntityAsset<Tbl = Self>,
    E::Key: Copy + Into<usize>,
{
    #[inline]
    fn get_mut(&mut self, k: &E::Key) -> Option<&mut E> {
        self.map.get_mut(k)
    }
}

impl<'a, E> IntoIterator for &'a VecTable<E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = VecTable<E>>,
{
    type Item = (&'a E::Key, &'a E);
    type IntoIter = Iter<'a, E::Key, E>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.map.iter()
    }
}

impl<'a, E> IntoParallelIterator for &'a VecTable<E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = VecTable<E>>,
{
    type Item = (&'a E::Key, &'a E);
    type Iter = ParIter<'a, E::Key, E>;

    #[inline]
    fn into_par_iter(self) -> Self::Iter {
        self.map.par_iter()
    }
}

impl<E: EntityAsset<Tbl = Self>> Tag for VecTable<E> {
    #[inline]
    fn tag(&self) -> VersionTag {
        self.tag
    }
}

pub struct VecTableTrx<'a, E: EntityAsset<Tbl = VecTable<E>>> {
    log_token: LogToken<Log<E>>,
    tbl: &'a VecTable<E>,
    trx: &'a mut Trx<'a>,
}

impl<'a, E> VecTableTrx<'a, E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = VecTable<E>>,
    E::Key: Copy + Eq + Hash,
    VecTable<E>: Asset<Log = Log<E>>,
    usize: From<E::Key>,
{
    pub fn get<'b>(&'b self, id: &E::Key) -> Option<&'b E> {
        match self
            .trx
            .log
            .get(&self.log_token)
            .and_then(|log| log.get(id))
        {
            Some(o) => o.as_ref(),
            None => self.tbl.get(id),
        }
    }

    pub fn get_owned(self, id: &E::Key) -> Option<&'a E> {
        match self
            .trx
            .log
            .get(&self.log_token)
            .and_then(|log| log.get(id))
        {
            Some(o) => o.as_ref(),
            None => self.tbl.get(id),
        }
    }

    pub fn insert<'b>(
        &'b mut self,
        id: E::Key,
        mut entity: E,
        track: &'b E::TrackCtx,
    ) -> impl Future<Output = Result<()>> + Send + use<'a, 'b, E>
    where
        E: EntityValidate + PartialEq,
        TransactionProvider<'a>: Upsert<E>,
    {
        Box::pin(async move {
            let gate = self.trx.err_gate.open()?;

            // if there is changes
            if self.get(&id).map_or(true, |old| *old != entity) {
                // raise change event & validate
                validate_on_change(self.trx, &id, &mut entity, track).await?;

                // if the change event revert incoming changes, do nothing.
                if self.get(&id).map_or(true, |current| *current != entity) {
                    self.trx.provider.upsert(&id, &entity).await?;

                    let old = self.tbl.get(&id);
                    entity.track_insert(&id, old, self.trx, track).await?;

                    E::changed().call(self.trx, &id, &entity, track).await?;
                    self.log_mut().insert(id, Some(entity));
                }
            }

            gate.close();

            Ok(())
        })
    }

    #[allow(clippy::manual_async_fn)]
    pub fn insert_mut<'b>(
        &'b mut self,
        mut id: E::Key,
        mut entity: E,
        track: &'b E::TrackCtx,
    ) -> impl Future<Output = Result<E::Key>> + Send + use<'a, 'b, E>
    where
        E: EntityValidate + PartialEq,
        TransactionProvider<'a>: UpsertMut<E>,
    {
        async move {
            let gate = self.trx.err_gate.open()?;

            // if there is changes
            if self.get(&id).map_or(true, |old| *old != entity) {
                // raise change event & validate
                validate_on_change(self.trx, &id, &mut entity, track).await?;

                // if the change event revert incoming changes, do nothing.
                if self.get(&id).map_or(true, |current| *current != entity) {
                    self.trx.provider.upsert_mut(&mut id, &mut entity).await?;

                    let old = self.tbl.get(&id);
                    entity.track_insert(&id, old, self.trx, track).await?;

                    E::changed().call(self.trx, &id, &entity, track).await?;
                    self.log_mut().insert(id, Some(entity));
                }
            }

            gate.close();

            Ok(id)
        }
    }

    #[allow(clippy::expect_used)]
    pub fn iter(&self) -> VecTableTrxIter<'_, E> {
        let map = self.trx.log.get(&self.log_token).expect("trx");

        VecTableTrxIter {
            ctx_iter: self.tbl.iter(),
            map,
            trx_iter: map.iter(),
        }
    }

    fn log_mut(&mut self) -> &mut Log<E> {
        self.trx.log.get_or_init_mut(&self.log_token)
    }

    #[allow(clippy::manual_async_fn)]
    pub fn remove<'b>(
        &'b mut self,
        id: E::Key,
        track: &'b E::TrackCtx,
    ) -> impl Future<Output = Result<()>> + Send + use<'a, 'b, E>
    where
        TransactionProvider<'a>: Delete<E>,
    {
        async move {
            let gate = self.trx.err_gate.open()?;

            if self.get(&id).is_some() {
                E::remove().call(self.trx, &id, track).await?;

                self.trx.provider.delete(&id).await?;

                if let Some(old) = self.tbl.get(&id) {
                    old.track_remove(&id, self.trx, track).await?;
                }

                E::removed().call(self.trx, &id, track).await?;
                self.log_mut().insert(id, None);
            }

            gate.close();

            Ok(())
        }
    }

    #[inline]
    pub async fn remove_filter<F>(&mut self, filter: F, track: &E::TrackCtx) -> Result<()>
    where
        F: FnMut(&E::Key, &E) -> bool,
        Self: Remove<E>,
    {
        Remove::remove_filter(self, filter, track).await
    }

    #[inline]
    pub async fn update_with<F>(&mut self, updater: F, track: &E::TrackCtx) -> Result<()>
    where
        E: PartialEq + ToOwned<Owned = E>,
        F: for<'c> FnMut(&'c E::Key, &'c mut Cow<E>),
        Self: Insert<E>,
    {
        Insert::update_with(self, updater, track).await
    }

    #[inline]
    pub async fn update_mut_with<F>(&mut self, updater: F, track: &E::TrackCtx) -> Result<()>
    where
        E: PartialEq + ToOwned<Owned = E>,
        F: for<'c> FnMut(&'c E::Key, &'c mut Cow<E>),
        Self: InsertMut<E>,
    {
        InsertMut::update_mut_with(self, updater, track).await
    }
}

impl<'a, E, Q> Get<E, Q> for VecTableTrx<'a, E>
where
    E: CtxTypeInfo + EntityAsset<Key = Q, Tbl = VecTable<E>>,
    Q: Copy + Eq + Hash,
    VecTable<E>: Asset<Log = Log<E>>,
    usize: From<Q>,
{
    #[inline]
    fn get_entity<'c>(&'c self, q: &Q) -> Option<&'c E> {
        VecTableTrx::get(self, q)
    }
}

impl<'a, E, Q> GetOwned<'a, E, Q> for VecTableTrx<'a, E>
where
    E: CtxTypeInfo + EntityAsset<Key = Q, Tbl = VecTable<E>>,
    Q: Copy + Eq + Hash,
    VecTable<E>: Asset<Log = Log<E>>,
    usize: From<Q>,
{
    #[inline]
    fn get_owned(self, q: &Q) -> Option<&'a E> {
        VecTableTrx::get_owned(self, q)
    }
}

impl<'a, E> Insert<E> for VecTableTrx<'a, E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = VecTable<E>> + EntityValidate + PartialEq,
    E::Key: Copy + Eq + Hash,
    VecTable<E>: Asset<Log = Log<E>>,
    TransactionProvider<'a>: Upsert<E>,
    usize: From<E::Key>,
{
    fn insert<'b>(
        &'b mut self,
        id: E::Key,
        entity: E,
        track: &'b E::TrackCtx,
    ) -> BoxFuture<'b, Result<()>> {
        Box::pin(VecTableTrx::insert(self, id, entity, track))
    }
}

impl<'a, E> InsertMut<E> for VecTableTrx<'a, E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = VecTable<E>> + EntityValidate + PartialEq,
    E::Key: Copy + Eq + Hash,
    VecTable<E>: Asset<Log = Log<E>>,
    TransactionProvider<'a>: UpsertMut<E>,
    usize: From<E::Key>,
{
    fn insert_mut<'c>(
        &'c mut self,
        id: E::Key,
        entity: E,
        track: &'c E::TrackCtx,
    ) -> BoxFuture<'c, Result<E::Key>> {
        Box::pin(VecTableTrx::insert_mut(self, id, entity, track))
    }
}

impl<'a, 'b, E> IntoIterator for &'b VecTableTrx<'a, E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = VecTable<E>>,
    E::Key: Copy + Eq + Hash,
    VecTable<E>: Asset<Log = Log<E>>,
    usize: From<E::Key>,
{
    type Item = (&'b E::Key, &'b E);
    type IntoIter = VecTableTrxIter<'b, E>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, E> Remove<E> for VecTableTrx<'a, E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = VecTable<E>>,
    E::Key: Copy + Eq + Hash,
    VecTable<E>: Asset<Log = Log<E>>,
    for<'c> TransactionProvider<'c>: Delete<E>,
    usize: From<E::Key>,
{
    fn remove<'b>(
        &'b mut self,
        id: <E as Entity>::Key,
        track: &'b <E as Entity>::TrackCtx,
    ) -> BoxFuture<'b, Result<()>> {
        Box::pin(VecTableTrx::remove(self, id, track))
    }
}

pub struct VecTableTrxIter<'a, E: Entity> {
    ctx_iter: vec_map::Iter<'a, E::Key, E>,
    map: &'a FxHashMap<E::Key, Option<E>>,
    trx_iter: std::collections::hash_map::Iter<'a, E::Key, Option<E>>,
}

impl<'a, E: Entity> Iterator for VecTableTrxIter<'a, E>
where
    E::Key: Eq + Hash,
{
    type Item = (&'a E::Key, &'a E);

    fn next(&mut self) -> Option<Self::Item> {
        for (k, v) in self.trx_iter.by_ref() {
            if let Some(v) = v.as_ref() {
                return Some((k, v));
            }
        }

        for (k, v) in self.ctx_iter.by_ref() {
            if !self.map.contains_key(k) {
                return Some((k, v));
            }
        }

        None
    }
}
