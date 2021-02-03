use crate::{Query, Result};
use async_trait::async_trait;

#[async_trait]
pub trait Load<P>: Sized {
    async fn load<C>(client: &C, params: &P) -> Result<Vec<Self>>
    where
        C: Query + Send + Sync;
}
