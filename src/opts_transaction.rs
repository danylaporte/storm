use crate::Result;
use async_trait::async_trait;

#[async_trait]
pub trait OptsTransaction {
    fn cancel(&self);

    async fn commit(&self) -> Result<()>;

    async fn transaction(&self) -> Result<()>;
}
