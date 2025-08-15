use crate::{
    provider::LoadAll, AsRefAsync, BoxFuture, Ctx, CtxTypeInfo, Entity, EntityAccessor, EntityOf,
    Gc, Get, GetMut, Logs, NotifyTag, ProviderContainer, RefIntoIterator, Result, Table, Tag,
    Touchable, TouchedEvent,
};
use fxhash::FxHashMap;
use rayon::{
    collections::hash_map::Iter as ParIter,
    iter::{IntoParallelIterator, IntoParallelRefIterator},
};
use std::{
    collections::hash_map::{Entry, Iter, Keys, Values},
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

    /// Used by the macro.
    #[doc(hidden)]
    pub fn __apply_log(ctx: &mut Ctx, logs: &mut Logs) -> bool
    where
        E: CtxTypeInfo + EntityAccessor<Tbl = HashTable<E>>,
    {
        let Some(log) = logs.remove(E::tbl_var()) else {
            return false;
        };

        if log.is_empty() {
            return false;
        }

        let Some(tbl) = ctx.ctx_ext_obj.get_mut(E::tbl_var()).get_mut() else {
            return false;
        };

        for (k, state) in log {
            match state {
                Some(new) => {
                    match tbl.map.entry(k) {
                        Entry::Occupied(mut o) => {
                            E::applied().call(o.key(), Some(o.get()), Some(&new));
                            o.insert(new);
                        }
                        Entry::Vacant(v) => {
                            E::applied().call(v.key(), None, Some(&new));
                            v.insert(new);
                        }
                    };
                }
                None => {
                    if let Some(old) = tbl.map.remove(&k) {
                        E::applied().call(&k, Some(&old), None);
                    }
                }
            }
        }

        tbl.update_metrics();
        tbl.tag.notify();
        true
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
        crate::telemetry::update_storm_table_rows(self.len(), E::NAME);
    }

    #[inline]
    pub fn values(&self) -> Values<E::Key, E> {
        self.map.values()
    }
}

impl<E> AsRefAsync<HashTable<E>> for Ctx
where
    E: EntityAccessor<Tbl = HashTable<E>> + CtxTypeInfo + Send,
    ProviderContainer: LoadAll<E, (), HashTable<E>>,
{
    #[inline]
    fn as_ref_async(&self) -> BoxFuture<'_, Result<&'_ HashTable<E>>> {
        E::tbl_from(self)
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
    E: CtxTypeInfo + Entity + Gc,
    E::Key: Eq + Hash,
{
    const SUPPORT_GC: bool = E::SUPPORT_GC;

    #[inline]
    fn gc(&mut self) {
        self.map.gc();
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

impl<E: EntityAccessor> Touchable for HashTable<E> {
    #[inline]
    fn touched() -> &'static TouchedEvent {
        E::touched()
    }
}

impl<E: Entity> RefIntoIterator for HashTable<E> {
    type Item<'a> = (&'a E::Key, &'a E);
    type Iter<'a> = Iter<'a, E::Key, E>;

    #[inline]
    fn ref_iter(&self) -> Self::Iter<'_> {
        self.iter()
    }
}

impl<E: CtxTypeInfo + Entity> Table<E> for HashTable<E> {
    #[inline]
    fn get(&self, key: &E::Key) -> Option<&E> {
        self.map.get(key)
    }
}
