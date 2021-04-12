use crate::{
    provider::LoadAll, state::State, version::version, ApplyLog, Entity, Get, GetMut, GetVersion,
    GetVersionOpt, Init, Log, MapTransaction, Result, Transaction,
};
use async_trait::async_trait;
use fxhash::FxHashMap;
use std::{
    collections::hash_map::{Iter, Keys, Values},
    hash::Hash,
    ops::Deref,
};

pub struct HashTable<E: Entity> {
    map: FxHashMap<E::Key, E>,
    version: u64,
}

impl<E: Entity> HashTable<E> {
    pub fn new() -> Self {
        Self {
            map: FxHashMap::default(),
            version: version(),
        }
    }

    pub fn iter(&self) -> Iter<E::Key, E> {
        self.map.iter()
    }

    pub fn keys(&self) -> Keys<E::Key, E> {
        self.map.keys()
    }

    pub fn values(&self) -> Values<E::Key, E> {
        self.map.values()
    }
}

impl<E: Entity> ApplyLog for HashTable<E>
where
    E::Key: Eq + Hash,
{
    type Log = Log<E>;

    fn apply_log(&mut self, log: Self::Log) {
        if log.is_empty() {
            self.version = version();
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

impl<E: Entity> Default for HashTable<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Entity> Deref for HashTable<E> {
    type Target = FxHashMap<E::Key, E>;

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
    fn get(&self, k: &E::Key) -> Option<&E> {
        self.map.get(k)
    }
}

impl<E: Entity> GetMut<E> for HashTable<E>
where
    E::Key: Eq + Hash,
{
    fn get_mut(&mut self, k: &E::Key) -> Option<&mut E> {
        self.map.get_mut(k)
    }
}

impl<E: Entity> GetVersion for HashTable<E> {
    fn get_version(&self) -> u64 {
        self.version
    }
}

impl<E: Entity> GetVersionOpt for HashTable<E> {
    fn get_version_opt(&self) -> Option<u64> {
        Some(self.version)
    }
}

#[async_trait]
impl<P, E> Init<P> for HashTable<E>
where
    E: Entity + Send,
    E::Key: Eq + Hash + Send,
    P: Sync + LoadAll<E, (), Self>,
{
    async fn init(provider: &P) -> Result<Self> {
        provider.load_all(&()).await
    }
}

impl<'a, E: Entity> IntoIterator for &'a HashTable<E> {
    type Item = (&'a E::Key, &'a E);
    type IntoIter = Iter<'a, E::Key, E>;

    fn into_iter(self) -> Self::IntoIter {
        self.map.iter()
    }
}

impl<'a, E: Entity> Transaction<'a> for HashTable<E>
where
    E: 'a,
{
    type Transaction = MapTransaction<E, &'a Self>;

    fn transaction(&'a self) -> Self::Transaction {
        MapTransaction::new(self)
    }
}
