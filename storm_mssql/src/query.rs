use async_trait::async_trait;
use std::{borrow::Cow, fmt::Debug};
use storm::Result;
use tiberius::{Row, ToSql};

#[async_trait]
pub trait Query {
    async fn query_rows<S>(&self, s: S, params: &[&(dyn ToSql)]) -> Result<Vec<Row>>
    where
        S: ?Sized + Debug + for<'a> Into<Cow<'a, str>> + Send;
}
