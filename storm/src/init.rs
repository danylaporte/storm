use crate::{provider::LoadAll, Entity, Result};
use async_trait::async_trait;
use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
};

#[async_trait]
pub trait Init<P>: Sized {
    async fn init(provider: &P) -> Result<Self>;
}

#[async_trait]
impl<P, E, S> Init<P> for HashMap<E::Key, E, S>
where
    E: Entity + Send,
    E::Key: Eq + Hash + Send,
    P: Sync + LoadAll<E>,
    S: BuildHasher + Default + Send,
{
    async fn init(provider: &P) -> Result<Self> {
        provider.load_all().await
    }
}

#[cfg(feature = "vec-map")]
#[async_trait]
impl<P, E> Init<P> for vec_map::VecMap<E::Key, E>
where
    E: Entity + Send,
    E::Key: Into<usize> + Send,
    P: Sync + LoadAll<E>,
{
    async fn init(provider: &P) -> Result<Self> {
        provider.load_all().await
    }
}
