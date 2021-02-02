use async_trait::async_trait;
use std::fmt::Debug;
use storm::Result;
use tokio_postgres::{types::ToSql, Row, ToStatement};

#[async_trait]
pub trait Query {
    async fn query_rows<S, P>(&self, s: &S, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>>
    where
        S: Debug + ToStatement + Send + Sync;
}
