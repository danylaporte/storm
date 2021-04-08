use crate::{
    provider::LoadAll, state::State, version::version, ApplyLog, Entity, Get, GetMut, GetVersion,
    GetVersionOpt, Init, Log, MapTransaction, Result, Transaction,
};
use async_trait::async_trait;
use std::ops::Deref;
use vec_map::{Keys, Values, VecMap};

pub struct VecTable<E: Entity> {
    map: VecMap<E::Key, E>,
    version: u64,
}

impl<E: Entity> VecTable<E> {
    pub fn new() -> Self {
        Self {
            map: VecMap::new(),
            version: version(),
        }
    }

    pub fn keys(&self) -> Keys<E::Key, E> {
        self.map.keys()
    }

    pub fn values(&self) -> Values<E::Key, E> {
        self.map.values()
    }
}

impl<E: Entity> ApplyLog for VecTable<E>
where
    E::Key: Clone + Into<usize>,
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

impl<E: Entity> Default for VecTable<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E: Entity> Deref for VecTable<E> {
    type Target = VecMap<E::Key, E>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl<E: Entity> Extend<(E::Key, E)> for VecTable<E>
where
    E::Key: Into<usize>,
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

impl<E: Entity> Get<E::Key, E> for VecTable<E>
where
    E::Key: Clone + Into<usize>,
{
    fn get(&self, k: &E::Key) -> Option<&E> {
        self.map.get(k)
    }
}

impl<E: Entity> GetMut<E> for VecTable<E>
where
    E::Key: Clone + Into<usize>,
{
    fn get_mut(&mut self, k: &E::Key) -> Option<&mut E> {
        self.map.get_mut(k)
    }
}

impl<E: Entity> GetVersion for VecTable<E> {
    fn get_version(&self) -> u64 {
        self.version
    }
}

impl<E: Entity> GetVersionOpt for VecTable<E> {
    fn get_version_opt(&self) -> Option<u64> {
        Some(self.version)
    }
}

#[async_trait]
impl<P, E> Init<P> for VecTable<E>
where
    E: Entity + Send,
    E::Key: Into<usize> + Send,
    P: Sync + LoadAll<E, (), Self>,
{
    async fn init(provider: &P) -> Result<Self> {
        provider.load_all(&()).await
    }
}

impl<'a, E: Entity> Transaction<'a> for VecTable<E>
where
    E: 'a,
{
    type Transaction = MapTransaction<E, &'a Self>;

    fn transaction(&'a self) -> Self::Transaction {
        MapTransaction::new(self)
    }
}
