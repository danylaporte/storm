use crate::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Commit {
    async fn commit(self) -> Result<()>;
}

#[async_trait]
impl Commit for () {
    async fn commit(self) -> Result<()> {
        Ok(())
    }
}
