use crate::{
    provider::LoadAll, state::State, Accessor, ApplyLog, BoxFuture, Deps, Entity, EntityAccessor,
    Get, GetMut, Init, Log, Result, Tag, TblVar,
};
use fxhash::FxHashMap;
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

    #[inline]
    pub fn values(&self) -> Values<E::Key, E> {
        self.map.values()
    }
}

impl<E> Accessor for HashTable<E>
where
    E: Entity + EntityAccessor<Coll = HashTable<E>>,
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
    E: Entity,
    E::Key: Eq + Hash,
{
    fn apply_log(&mut self, log: Log<E>) {
        if !log.is_empty() {
            self.tag.notify();
        }

        for (k, state) in log {
            match state {
                State::Inserted(v) => {
                    self.map.insert(k, v);
                }
                State::Removed => {
                    self.map.remove(&k);
                }
            }
        }
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

impl<E: Entity> Extend<(E::Key, E)> for HashTable<E>
where
    E::Key: Eq + Hash,
{
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = (E::Key, E)>,
    {
        for (k, v) in iter {
            self.map.insert(k, v);
        }
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
    E: Entity + Send,
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

impl<E: Entity> Tag for HashTable<E> {
    fn tag(&self) -> VersionTag {
        self.tag
    }
}
