use crate::{BoxFuture, Result};
use std::any::Any;

pub trait Provider: Any + Send + Sync {
    fn cancel(&self);
    fn commit<'a>(&'a self) -> BoxFuture<'a, Result<()>>;
}
