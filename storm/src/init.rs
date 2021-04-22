use crate::{BoxFuture, Result};

pub trait Init<'a, P>: Sized + Send + Sync {
    fn init(provider: &'a P) -> BoxFuture<'a, Result<Self>>;
}
