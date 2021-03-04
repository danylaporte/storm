use async_trait::async_trait;

#[async_trait]
pub trait GetOrLoad<T, C> {
    fn get_or_load<'a>(&'a self, ctx: &C) -> &'a T;

    fn get_mut(&mut self) -> Option<&mut T>;
}
