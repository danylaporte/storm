use crate::Result;
use async_trait::async_trait;

#[async_trait]
pub trait RowLoad<O>: Sized {
    async fn row_load(opts: &O) -> Result<Vec<Self>>;
}
