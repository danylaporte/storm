use crate::{
    mem,
    provider::{Delete, LoadOne, Upsert},
    Entity, Get, Log, Result, State,
};
use std::hash::{BuildHasher, Hash};

pub struct MapTransaction<E: Entity, M> {
    log: Log<E>,
    map: M,
}

impl<E: Entity, M> MapTransaction<E, M> {
    pub fn new(map: M) -> Self {
        Self {
            log: Default::default(),
            map,
        }
    }
}

impl<E: Entity, M> MapTransaction<E, M>
where
    E::Key: Eq + Hash,
{
    pub fn get(&self, k: &E::Key) -> Option<&E>
    where
        M: Get<E::Key, E>,
    {
        match self.log.get(k) {
            Some(State::Inserted(v)) => Some(v),
            Some(State::Removed) => None,
            None => self.map.get(k),
        }
    }

    pub async fn insert<P>(&mut self, k: E::Key, v: E, provider: &P) -> Result<()>
    where
        P: Upsert<E>,
    {
        provider.upsert(&k, &v).await?;
        self.log.insert(k, State::Inserted(v));
        Ok(())
    }

    pub async fn remove<P>(&mut self, k: E::Key, provider: &P) -> Result<()>
    where
        P: Delete<E>,
    {
        provider.delete(&k).await?;
        self.log.insert(k, State::Removed);
        Ok(())
    }
}

impl<E: Entity, M> Get<E::Key, E> for MapTransaction<E, M>
where
    E::Key: Eq + Hash,
    M: Get<E::Key, E>,
{
    fn get(&self, k: &E::Key) -> Option<&E> {
        Self::get(self, k)
    }
}

#[cfg(feature = "cache")]
impl<E: Entity, S> MapTransaction<E, cache::Cache<E::Key, E, S>>
where
    E::Key: Clone + Eq + Hash,
    S: BuildHasher,
{
    pub async fn get_or_load<P>(&mut self, k: &E::Key, provider: &P) -> Result<Option<&E>>
    where
        P: LoadOne<E>,
    {
        self.load_key(k, provider).await?;

        Ok(match self.log.get(k) {
            Some(State::Inserted(v)) => Some(v),
            Some(State::Removed) => None,
            None => self.map.get(k),
        })
    }

    pub async fn load_key<P>(&mut self, k: &E::Key, provider: &P) -> Result<()>
    where
        P: LoadOne<E>,
    {
        if !self.log.contains_key(k) && !self.map.contains_key(k) {
            if let Some(v) = provider.load_one(k).await? {
                self.log.insert(k.clone(), State::Inserted(v));
            }
        }

        Ok(())
    }
}

impl<E: Entity, M> mem::Commit for MapTransaction<E, M> {
    type Log = Log<E>;

    fn commit(self) -> Self::Log {
        self.log
    }
}
