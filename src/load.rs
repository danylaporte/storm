use crate::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Load<O>: Sized {
    async fn load(opts: &O) -> Result<Vec<Self>>;
}
