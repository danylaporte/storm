use async_trait::async_trait;
use postgres_types::ToSql;
use std::fmt::Debug;
use storm::Result;
use tokio_postgres::ToStatement;

#[async_trait]
pub trait Execute {
    async fn execute<S>(&self, s: &S, params: &[&(dyn ToSql + Sync)]) -> Result<u64>
    where
        S: ?Sized + Debug + ToStatement + Send + Sync;
}
