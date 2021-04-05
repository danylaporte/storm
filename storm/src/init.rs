use crate::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Init<P>: Sized {
    async fn init(provider: &P) -> Result<Self>;
}
