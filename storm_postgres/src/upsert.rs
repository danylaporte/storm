use crate::{Execute, Result};
use async_trait::async_trait;

#[async_trait]
pub trait Upsert {
    async fn upsert<C>(&self, client: &C) -> Result<u64>
    where
        C: Execute + Send + Sync;
}
