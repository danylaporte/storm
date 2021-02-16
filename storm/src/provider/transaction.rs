use crate::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Transaction {
    async fn transaction(&self) -> Result<()>;
}
