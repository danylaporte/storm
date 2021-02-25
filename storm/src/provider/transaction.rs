use crate::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Transaction<'a> {
    type Transaction;

    async fn transaction(&'a self) -> Result<Self::Transaction>;
}

#[async_trait]
impl<'a> Transaction<'a> for () {
    type Transaction = ();
    async fn transaction(&'a self) -> Result<Self::Transaction> {
        Ok(())
    }
}
