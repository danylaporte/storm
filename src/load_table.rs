use crate::{Entity, Error, Result, Row, RowLoad, Table};
use async_trait::async_trait;
use std::{
    convert::{TryFrom, TryInto},
    iter::FromIterator,
};

#[async_trait]
pub trait LoadTable<O>: Sized {
    async fn load_table(opts: &O) -> Result<Self>;
}

#[async_trait]
impl<O, T> LoadTable<O> for T
where
    O: Send + Sync,
    T: Table + FromIterator<(<<T as Table>::Entity as Entity>::Key, <T as Table>::Entity)>,
    T::Entity: TryFrom<<T::Entity as Entity>::Row, Error = Error>,
    <T::Entity as Entity>::Key: From<<<T::Entity as Entity>::Row as Row>::Key>,
    <T::Entity as Entity>::Row: RowLoad<O>,
{
    async fn load_table(opts: &O) -> Result<Self> {
        let items = <<T::Entity as Entity>::Row as RowLoad<O>>::row_load(opts).await?;

        items
            .into_iter()
            .map(|r| Ok((r.key().into(), r.try_into()?)))
            .collect()
    }
}
