use crate::{
    provider::LoadAll, Accessor, ApplyLog, BoxFuture, CtxTypeInfo, Deps, Entity, EntityAccessor,
    EntityOf, Gc, GcCtx, Get, GetMut, Init, Log, LogState, NotifyTag, Result, Tag, TblVar,
};
use fxhash::FxHashMap;
use rayon::{
    collections::hash_map::Iter as ParIter,
    iter::{IntoParallelIterator, IntoParallelRefIterator},
};
use std::{
    collections::hash_map::{Iter, Keys, Values},
    hash::Hash,
    ops::Deref,
};
use version_tag::VersionTag;

pub struct HashTable<E: Entity> {
    map: FxHashMap<E::Key, E>,
    tag: VersionTag,
}

impl<E: Entity> HashTable<E> {
    pub fn new() -> Self {
        Self {
            map: FxHashMap::default(),
            tag: VersionTag::new(),
        }
    }

    #[inline]
    pub fn iter(&self) -> Iter<E::Key, E> {
        self.map.iter()
    }

    #[inline]
    pub fn keys(&self) -> Keys<E::Key, E> {
        self.map.keys()
    }

    fn update_metrics(&self)
    where
        E: CtxTypeInfo,
    {
        #[cfg(feature = "telemetry")]
        {
            use conv::ValueFrom;
            metrics::gauge!("storm_table_rows", f64::value_from(self.len()).unwrap_or(0.0), "type" => E::NAME);
        }
    }

    #[inline]
    pub fn values(&self) -> Values<E::Key, E> {
        self.map.values()
    }
}

impl<E> Accessor for HashTable<E>
where
    E: Entity + EntityAccessor<Tbl = HashTable<E>>,
{
    #[inline]
    fn var() -> &'static TblVar<Self> {
        E::entity_var()
    }

    #[inline]
    fn deps() -> &'static Deps {
        E::entity_deps()
    }
}

impl<E> ApplyLog<Log<E>> for HashTable<E>
where
    E: CtxTypeInfo + Entity + EntityAccessor<Tbl = Self>,
    E::Key: Eq + Hash,
{
    fn apply_log(&mut self, log: Log<E>) -> bool {
        if log.is_empty() {
            return false;
        }

        for (k, state) in log {
            match state {
                LogState::Inserted(v) => {
                    self.map.insert(k, v);
                }
                LogState::Removed => {
                    self.map.remove(&k);
                }
            }
        }

        self.update_metrics();
        self.tag.notify();
        true
    }
}

impl<E: Entity> AsRef<Self> for HashTable<E> {
    #[inline]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl<E: Entity> Default for HashTable<E> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Entity> Deref for HashTable<E> {
    type Target = FxHashMap<E::Key, E>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl<E: Entity> EntityOf for HashTable<E> {
    type Entity = E;
}

impl<E> Extend<(E::Key, E)> for HashTable<E>
where
    E: CtxTypeInfo + Entity,
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

impl<E> Gc for HashTable<E>
where
    E: Entity + Gc,
    E::Key: Eq + Hash,
{
    const SUPPORT_GC: bool = E::SUPPORT_GC;

    #[inline]
    fn gc(&mut self, ctx: &GcCtx) {
        self.map.gc(ctx);
    }
}

impl<E: Entity> Get<E> for HashTable<E>
where
    E::Key: Eq + Hash,
{
    #[inline]
    fn get(&self, k: &E::Key) -> Option<&E> {
        self.map.get(k)
    }
}

impl<E: Entity> GetMut<E> for HashTable<E>
where
    E::Key: Eq + Hash,
{
    #[inline]
    fn get_mut(&mut self, k: &E::Key) -> Option<&mut E> {
        self.map.get_mut(k)
    }
}

impl<'a, P, E> Init<'a, P> for HashTable<E>
where
    E: CtxTypeInfo + Entity + Send,
    E::Key: Eq + Hash + Send,
    P: Sync + LoadAll<E, (), Self>,
{
    #[inline]
    fn init(provider: &'a P) -> BoxFuture<'a, Result<Self>> {
        provider.load_all(&())
    }
}

impl<'a, E: Entity> IntoIterator for &'a HashTable<E> {
    type Item = (&'a E::Key, &'a E);
    type IntoIter = Iter<'a, E::Key, E>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.map.iter()
    }
}

impl<'a, E> IntoParallelIterator for &'a HashTable<E>
where
    E: Entity,
    E::Key: Eq + Hash,
{
    type Item = (&'a E::Key, &'a E);
    type Iter = ParIter<'a, E::Key, E>;

    fn into_par_iter(self) -> Self::Iter {
        self.par_iter()
    }
}

impl<E: Entity> NotifyTag for HashTable<E> {
    fn notify_tag(&mut self) {
        self.tag.notify()
    }
}

impl<E: Entity> Tag for HashTable<E> {
    fn tag(&self) -> VersionTag {
        self.tag
    }
}
