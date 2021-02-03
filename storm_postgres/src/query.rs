use async_trait::async_trait;
use postgres_types::ToSql;
use std::fmt::Debug;
use storm::Result;
use tokio_postgres::{Row, ToStatement};

#[async_trait]
pub trait Query {
    async fn query_rows<S>(&self, s: &S, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>>
    where
        S: ?Sized + Debug + ToStatement + Send + Sync;
}
