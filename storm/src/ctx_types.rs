use crate::{mem, Entity};
use once_cell::sync::OnceCell;

pub trait CtxTypes<'a> {
    type Output;
    type Transaction;
}

#[cfg(feature = "cache")]
impl<'a, E, S> CtxTypes<'a> for cache::Cache<E::Key, E, S>
where
    E: Entity + 'a,
    S: 'a,
{
    type Output = Self;
    type Transaction = <Self as mem::Transaction<'a>>::Transaction;
}

impl<'a, T: 'a> CtxTypes<'a> for OnceCell<T>
where
    T: mem::Transaction<'a>,
{
    type Output = T;
    type Transaction = <T as mem::Transaction<'a>>::Transaction;
}
