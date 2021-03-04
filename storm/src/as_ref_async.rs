use crate::{Connected, GetOrLoadAsync, Result};
use async_trait::async_trait;

#[async_trait]
pub trait AsRefAsync<T> {
    async fn as_ref_async(&self) -> Result<&T>;
}

#[async_trait]
impl<C, P, T> AsRefAsync<T> for Connected<C, P>
where
    C: GetOrLoadAsync<T, P> + Send + Sync,
    P: Send + Sync,
    T: 'static,
{
    async fn as_ref_async(&self) -> Result<&T> {
        GetOrLoadAsync::get_or_load_async(&self.ctx, &self.provider).await
    }
}
