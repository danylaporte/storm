use crate::{
    log::LogToken,
    provider::{Delete, LoadAll, LoadArgs, TransactionProvider, Upsert, UpsertMut},
    validate_on_change, Asset, AssetBase, BoxFuture, CtxTypeInfo, CtxVars, Entity, EntityAsset,
    EntityValidate, Get, GetMut, GetOwned, Insert, InsertMut, LogVars, NotifyTag,
    ProviderContainer, Remove, Result, Tag, Trx,
};
use attached::Var;
use fxhash::FxHashMap;
use rayon::{
    collections::hash_map::Iter as ParIter,
    iter::{IntoParallelIterator, IntoParallelRefIterator},
};
use std::{
    borrow::{Borrow, Cow},
    collections::hash_map::{self, Entry, Iter, Keys, Values},
    future::Future,
    hash::Hash,
};
use version_tag::VersionTag;

type Log<E> = FxHashMap<<E as Entity>::Key, Option<E>>;

pub struct HashTable<E: EntityAsset> {
    map: FxHashMap<E::Key, E>,
    tag: VersionTag,
}

impl<E> HashTable<E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = Self>,
    E::Key: Eq + Hash,
{
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    #[inline]
    pub fn get<'a, Q>(&'a self, key: &Q) -> Option<&'a E>
    where
        E::Key: Borrow<Q>,
        Q: Eq + Hash,
    {
        self.map.get(key)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    #[inline]
    pub fn keys(&self) -> Keys<E::Key, E> {
        self.map.keys()
    }

    #[inline]
    pub fn iter(&self) -> Iter<E::Key, E> {
        self.map.iter()
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

impl<E> AssetBase for HashTable<E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = Self> + PartialEq + 'static,
    E::Key: Eq + Hash,
    ProviderContainer: LoadAll<E, (), Self>,
{
    type Log = Log<E>;
    type Trx<'a: 'b, 'b> = HashTableTrx<'a, 'b, E>;

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
            }
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

    fn trx<'a: 'b, 'b>(
        &'b self,
        trx: &'b mut Trx<'a>,
        log_token: LogToken<Log<E>>,
    ) -> Self::Trx<'a, 'b> {
        HashTableTrx {
            log_token,
            tbl: self,
            trx,
        }
    }
}

impl<E> Asset for HashTable<E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = HashTable<E>> + PartialEq,
    E::Key: Eq + Hash,
    ProviderContainer: LoadAll<E, (), Self>,
{
    #[inline]
    fn ctx_var() -> Var<Self, CtxVars> {
        E::ctx_var()
    }

    #[inline]
    fn log_var() -> Var<Self::Log, LogVars> {
        E::log_var()
    }

    #[inline]
    fn init(ctx: &crate::Ctx) -> impl Future<Output = Result<Self>> + Send {
        ctx.provider.load_all_with_args(&(), LoadArgs::default())
    }
}

impl<E: EntityAsset<Tbl = Self>> Default for HashTable<E> {
    #[inline]
    fn default() -> Self {
        Self {
            map: FxHashMap::default(),
            tag: VersionTag::new(),
        }
    }
}

impl<E> Extend<(E::Key, E)> for HashTable<E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = Self>,
    E::Key: Eq + Hash,
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

impl<E, Q> Get<E, Q> for HashTable<E>
where
    E: EntityAsset<Tbl = Self>,
    E::Key: Borrow<Q> + Eq + Hash,
    Q: Eq + Hash,
{
    #[inline]
    fn get_entity<'a>(&'a self, q: &Q) -> Option<&'a E> {
        self.map.get(q)
    }
}

impl<E> GetMut<E> for HashTable<E>
where
    E: EntityAsset<Tbl = Self>,
    E::Key: Eq + Hash,
{
    #[inline]
    fn get_mut(&mut self, k: &E::Key) -> Option<&mut E> {
        self.map.get_mut(k)
    }
}

impl<'a, E: EntityAsset<Tbl = Self>> IntoIterator for &'a HashTable<E> {
    type Item = (&'a E::Key, &'a E);
    type IntoIter = Iter<'a, E::Key, E>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.map.iter()
    }
}

impl<'a, E> IntoParallelIterator for &'a HashTable<E>
where
    E: EntityAsset<Tbl = Self>,
    E::Key: Eq + Hash,
{
    type Item = (&'a E::Key, &'a E);
    type Iter = ParIter<'a, E::Key, E>;

    fn into_par_iter(self) -> Self::Iter {
        self.par_iter()
    }
}

impl<E: EntityAsset<Tbl = Self>> NotifyTag for HashTable<E> {
    fn notify_tag(&mut self) {
        self.tag.notify()
    }
}

impl<E: EntityAsset<Tbl = Self>> Tag for HashTable<E> {
    fn tag(&self) -> VersionTag {
        self.tag
    }
}

pub struct HashTableTrx<'a, 'b, E: EntityAsset<Tbl = HashTable<E>>> {
    log_token: LogToken<Log<E>>,
    tbl: &'b HashTable<E>,
    trx: &'b mut Trx<'a>,
}

impl<'a, 'b, E> HashTableTrx<'a, 'b, E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = HashTable<E>> + PartialEq,
    E::Key: Eq + Hash,
    HashTable<E>: AssetBase<Log = Log<E>>,
{
    #[inline]
    pub fn get<Q>(&self, q: &Q) -> Option<&E>
    where
        E::Key: Borrow<Q>,
        Q: Eq + Hash,
    {
        match self.trx.log.get(&self.log_token).and_then(|log| log.get(q)) {
            Some(o) => o.as_ref(),
            None => self.tbl.get(q),
        }
    }

    pub fn get_owned<Q>(self, q: &Q) -> Option<&'b E>
    where
        E::Key: Borrow<Q>,
        Q: Eq + Hash,
    {
        match self.trx.log.get(&self.log_token).and_then(|log| log.get(q)) {
            Some(o) => o.as_ref(),
            None => self.tbl.get(q),
        }
    }

    pub async fn insert(&mut self, id: E::Key, mut entity: E, track: &E::TrackCtx) -> Result<()>
    where
        E: EntityValidate,
        TransactionProvider<'a>: Upsert<E>,
    {
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
    }

    pub async fn insert_mut(
        &mut self,
        mut id: E::Key,
        mut entity: E,
        track: &E::TrackCtx,
    ) -> Result<E::Key>
    where
        E: EntityValidate,
        E::Key: Clone,
        TransactionProvider<'a>: UpsertMut<E>,
    {
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
                self.log_mut().insert(id.clone(), Some(entity));
            }
        }

        gate.close();

        Ok(id)
    }

    pub fn iter(&self) -> HashTableTrxIter<'_, E> {
        let map = self.trx.log.get(&self.log_token).expect("trx");

        HashTableTrxIter {
            ctx_iter: self.tbl.iter(),
            map,
            trx_iter: map.iter(),
        }
    }

    fn log_mut(&mut self) -> &mut Log<E> {
        self.trx.log.get_or_init_mut(&self.log_token)
    }

    pub async fn remove(&mut self, id: E::Key, track: &E::TrackCtx) -> Result<()>
    where
        TransactionProvider<'a>: Delete<E>,
    {
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

    #[inline]
    pub async fn remove_filter<F>(&mut self, filter: F, track: &E::TrackCtx) -> Result<()>
    where
        E::Key: Clone,
        F: FnMut(&E::Key, &E) -> bool,
        Self: Remove<E>,
    {
        Remove::remove_filter(self, filter, track).await
    }

    #[inline]
    pub async fn update_with<F>(&mut self, updater: F, track: &E::TrackCtx) -> Result<()>
    where
        E: PartialEq + ToOwned<Owned = E>,
        E::Key: Clone,
        F: for<'c> FnMut(&'c E::Key, &'c mut Cow<E>),
        Self: Insert<E>,
    {
        Insert::update_with(self, updater, track).await
    }

    #[inline]
    pub async fn update_mut_with<F>(&mut self, updater: F, track: &E::TrackCtx) -> Result<()>
    where
        E: PartialEq + ToOwned<Owned = E>,
        E::Key: Clone,
        F: for<'c> FnMut(&'c E::Key, &'c mut Cow<E>),
        Self: InsertMut<E>,
    {
        InsertMut::update_mut_with(self, updater, track).await
    }
}

impl<'a, 'b, E, Q> Get<E, Q> for HashTableTrx<'a, 'b, E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = HashTable<E>> + PartialEq,
    E::Key: Borrow<Q> + Eq + Hash,
    HashTable<E>: AssetBase<Log = Log<E>>,
    Q: Eq + Hash,
{
    #[inline]
    fn get_entity<'c>(&'c self, q: &Q) -> Option<&'c E> {
        self.get(q)
    }
}

impl<'a, 'b, Q, E> GetOwned<'b, E, Q> for HashTableTrx<'a, 'b, E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = HashTable<E>> + PartialEq,
    E::Key: Borrow<Q> + Eq + Hash,
    HashTable<E>: AssetBase<Log = Log<E>>,
    Q: Eq + Hash,
{
    #[inline]
    fn get_owned(self, q: &Q) -> Option<&'b E> {
        self.get_owned(q)
    }
}

impl<'a, 'b, E> Insert<E> for HashTableTrx<'a, 'b, E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = HashTable<E>> + EntityValidate + PartialEq,
    E::Key: Eq + Hash,
    HashTable<E>: AssetBase<Log = Log<E>>,
    for<'c> TransactionProvider<'c>: Upsert<E>,
{
    fn insert<'c>(
        &'c mut self,
        id: E::Key,
        entity: E,
        track: &'c E::TrackCtx,
    ) -> BoxFuture<'c, Result<()>> {
        Box::pin(self.insert(id, entity, track))
    }
}

impl<'a, 'b, E> InsertMut<E> for HashTableTrx<'a, 'b, E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = HashTable<E>> + EntityValidate + PartialEq,
    E::Key: Clone + Eq + Hash,
    HashTable<E>: AssetBase<Log = Log<E>>,
    for<'c> TransactionProvider<'c>: UpsertMut<E>,
{
    fn insert_mut<'c>(
        &'c mut self,
        id: E::Key,
        entity: E,
        track: &'c E::TrackCtx,
    ) -> BoxFuture<'c, Result<E::Key>> {
        Box::pin(self.insert_mut(id, entity, track))
    }
}

impl<'a, 'b, 'c, E> IntoIterator for &'c HashTableTrx<'a, 'b, E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = HashTable<E>> + PartialEq,
    E::Key: Eq + Hash,
    HashTable<E>: AssetBase<Log = Log<E>>,
{
    type Item = (&'c E::Key, &'c E);
    type IntoIter = HashTableTrxIter<'c, E>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, 'b, E> Remove<E> for HashTableTrx<'a, 'b, E>
where
    E: CtxTypeInfo + EntityAsset<Tbl = HashTable<E>> + PartialEq,
    E::Key: Eq + Hash,
    HashTable<E>: AssetBase<Log = Log<E>>,
    for<'c> TransactionProvider<'c>: Delete<E>,
{
    fn remove<'c>(
        &'c mut self,
        id: <E as Entity>::Key,
        track: &'c <E as Entity>::TrackCtx,
    ) -> BoxFuture<'c, Result<()>> {
        Box::pin(self.remove(id, track))
    }
}

pub struct HashTableTrxIter<'a, E: Entity> {
    ctx_iter: hash_map::Iter<'a, E::Key, E>,
    map: &'a Log<E>,
    trx_iter: hash_map::Iter<'a, E::Key, Option<E>>,
}

impl<'a, E> Iterator for HashTableTrxIter<'a, E>
where
    E: Entity,
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
