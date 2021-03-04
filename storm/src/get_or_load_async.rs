use crate::{provider::Gate, Init, Result};
use async_trait::async_trait;
use once_cell::sync::OnceCell;

#[async_trait]
pub trait GetOrLoadAsync<T, P> {
    async fn get_or_load_async<'a>(&'a self, provider: &P) -> Result<&'a T>;

    fn get_mut(&mut self) -> Option<&mut T>;
}

/// OnceCell requires the context to implement a gate so that there won't be
/// multiple futures that initialize the value.
#[async_trait]
impl<P, T> GetOrLoadAsync<T, P> for OnceCell<T>
where
    P: for<'b> Gate<'b> + Sync,
    T: Init<P> + Send + Sync,
{
    async fn get_or_load_async<'a>(&'a self, provider: &P) -> Result<&'a T> {
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

    fn get_mut(&mut self) -> Option<&mut T> {
        self.get_mut()
    }
}
