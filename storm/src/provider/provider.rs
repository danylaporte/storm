use crate::Result;
use async_trait::async_trait;
use std::any::Any;

#[async_trait]
pub trait Provider: Any + Send + Sync {
    fn cancel(&self);
    async fn commit(&self) -> Result<()>;
}
