use crate::{Entity, EntityLoad, Result, Table};
use async_trait::async_trait;
use std::iter::FromIterator;

#[async_trait]
pub trait LoadTable<O>: Sized {
    async fn load_table(opts: &O) -> Result<Self>;
}

#[async_trait]
impl<O, T> LoadTable<O> for T
where
    O: Send + Sync,
    T: Table + FromIterator<(<<T as Table>::Entity as Entity>::Key, <T as Table>::Entity)>,
    T::Entity: EntityLoad<O>,
{
    async fn load_table(opts: &O) -> Result<Self> {
        Ok(EntityLoad::entity_load(opts).await?.into_iter().collect())
    }
}
