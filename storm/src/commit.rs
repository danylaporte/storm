use crate::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Commit {
    type Log;

    async fn commit(self) -> Result<Self::Log>;
}
