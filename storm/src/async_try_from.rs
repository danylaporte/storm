use crate::{BoxFuture, Result};

pub trait AsyncTryFrom<'a, T>: Sized {
    fn async_try_from(t: T) -> BoxFuture<'a, Result<Self>>;
}
