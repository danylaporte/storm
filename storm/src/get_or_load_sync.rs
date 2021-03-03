use async_trait::async_trait;

#[async_trait]
pub trait GetOrLoadSync<T, C> {
    fn get_or_load_sync<'a>(&'a self, ctx: &C) -> &'a T;

    fn get_mut(&mut self) -> Option<&mut T>;
}
