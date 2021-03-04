use crate::{AsyncOnceCell, Init, Result};
use async_trait::async_trait;

#[async_trait]
pub trait GetOrLoadAsync<T, P> {
    async fn get_or_load_async<'a>(&'a self, provider: &P) -> Result<&'a T>
    where
        T: 'a;
}

#[async_trait]
impl<C, T, P> GetOrLoadAsync<T, P> for &C
where
    C: GetOrLoadAsync<T, P> + Send + Sync,
    P: Send + Sync,
    T: Send + Sync,
{
    async fn get_or_load_async<'a>(&'a self, provider: &P) -> Result<&'a T>
    where
        T: 'a,
    {
        (**self).get_or_load_async(provider).await
    }
}

#[async_trait]
impl<P, T> GetOrLoadAsync<T, P> for AsyncOnceCell<T>
where
    P: Sync,
    T: Init<P> + Send + Sync,
{
    async fn get_or_load_async<'a>(&'a self, provider: &P) -> Result<&'a T>
    where
        T: 'a,
    {
        self.get_or_try_init(T::init(provider)).await
    }
}
