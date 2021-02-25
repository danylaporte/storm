use crate::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Commit {
    type Log;

    #[must_use]
    async fn commit(self) -> Result<Self::Log>;
}
