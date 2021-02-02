use crate::{EntitiesLoad, Entity, Result, Table};
use async_trait::async_trait;
use cache::Cache;
use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
};

#[async_trait]
pub trait TableLoad<O>: Table {
    async fn table_load(opts: &O) -> Result<Self>;
}

#[cfg(feature = "cache")]
#[async_trait]
impl<K, O, S, V> TableLoad<O> for cache::Cache<K, V, S>
where
    K: Eq + Hash,
    O: Send + Sync,
    S: BuildHasher + Default,
    V: Entity<Key = K>,
{
    async fn table_load(_opts: &O) -> Result<Self> {
        Ok(Cache::default())
    }
}

#[async_trait]
impl<K, O, S, V> TableLoad<O> for HashMap<K, V, S>
where
    K: Eq + Hash,
    O: Send + Sync,
    S: BuildHasher + Default,
    V: Entity<Key = K> + EntitiesLoad<O>,
{
    async fn table_load(opts: &O) -> Result<Self> {
        Ok(V::entities_load(opts).await?.into_iter().collect())
    }
}

#[cfg(feature = "vec-map")]
#[async_trait]
impl<K, O, V> TableLoad<O> for vec_map::VecMap<K, V>
where
    K: Clone + Into<usize>,
    O: Send + Sync,
    V: Entity<Key = K> + EntitiesLoad<O>,
{
    async fn table_load(opts: &O) -> Result<Self> {
        Ok(V::entities_load(opts).await?.into_iter().collect())
    }
}
