use crate::{provider::Gate, Init, Result};
use async_trait::async_trait;
use once_cell::sync::OnceCell;

#[async_trait]
pub trait GetOrLoad<P> {
    type Output;

    async fn get_or_load<'a>(&'a self, provider: &P) -> Result<&'a Self::Output>;

    fn get_mut(&mut self) -> Option<&mut Self::Output>;
}

/// OnceCell requires the context to implement a gate so that there won't be
/// multiple futures that initialize the value.
#[async_trait]
impl<P, T> GetOrLoad<P> for OnceCell<T>
where
    P: for<'b> Gate<'b> + Sync,
    T: Init<P> + Send + Sync,
{
    type Output = T;

    async fn get_or_load<'a>(&'a self, provider: &P) -> Result<&'a Self::Output> {
        if let Some(v) = self.get() {
            return Ok(v);
        }

        let _gate = provider.gate().await;

        if let Some(v) = self.get() {
            return Ok(v);
        }

        let r = T::init(provider).await?;

        Ok(self.get_or_init(|| r))
    }

    fn get_mut(&mut self) -> Option<&mut Self::Output> {
        self.get_mut()
    }
}
