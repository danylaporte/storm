use async_trait::async_trait;
use std::{borrow::Cow, fmt::Debug};
use storm::Result;
use tiberius::ToSql;

#[async_trait]
pub trait Execute {
    async fn execute<'a, S>(&self, s: S, params: &[&(dyn ToSql)]) -> Result<u64>
    where
        S: ?Sized + Debug + Into<Cow<'a, str>> + Send;
}
