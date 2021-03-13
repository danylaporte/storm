use crate::{provider::ProviderContainer, Connected, ConnectedRef, GetOrLoadAsync, Result};
use async_trait::async_trait;

#[async_trait]
pub trait AsRefAsync<'a, T> {
    async fn as_ref_async(&'a self) -> Result<&'a T>;
}

#[async_trait]
impl<'a, C, T> AsRefAsync<'a, T> for Connected<C>
where
    C: GetOrLoadAsync<T, ProviderContainer> + Send + Sync,
    T: 'a,
{
    async fn as_ref_async(&'a self) -> Result<&'a T> {
        GetOrLoadAsync::get_or_load_async(&self.ctx, &self.provider).await
    }
}

#[async_trait]
impl<'a, 'b, C, T> AsRefAsync<'a, T> for ConnectedRef<'b, C>
where
    C: GetOrLoadAsync<T, ProviderContainer> + Send + Sync,
    T: 'a,
{
    async fn as_ref_async(&'a self) -> Result<&'a T> {
        GetOrLoadAsync::get_or_load_async(&self.ctx, &self.provider).await
    }
}
