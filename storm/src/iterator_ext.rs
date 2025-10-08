use crate::{
    provider::{Delete, LoadAll, TransactionProvider, Upsert},
    BoxFuture, CtxTransaction, EntityRemove, EntityUpsert, ProviderContainer, Result,
};

pub trait IteratorExt: Iterator {
    fn insert_all<'a, 'b, E>(self, trx: &'b mut CtxTransaction<'a>) -> BoxFuture<'b, Result<usize>>
    where
        'a: 'b,
        E: EntityUpsert,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        Self: Iterator<Item = (E::Key, E)> + Sized,
        for<'c> TransactionProvider<'c>: Upsert<E>,
    {
        trx.insert_all(self)
    }

    fn remove_all<'a, 'b, E>(self, trx: &'b mut CtxTransaction<'a>) -> BoxFuture<'b, Result<usize>>
    where
        'a: 'b,
        E: EntityRemove,
        ProviderContainer: LoadAll<E, (), E::Tbl>,
        Self: Iterator<Item = E::Key> + Sized + Send + 'b,
        for<'c> TransactionProvider<'c>: Delete<E>,
    {
        trx.remove_all(self)
    }
}

impl<T> IteratorExt for T where T: Iterator {}

pub trait RefIntoIterator {
    type Item<'a>
    where
        Self: 'a;

    type Iter<'a>: Iterator<Item = Self::Item<'a>>
    where
        Self: 'a;

    fn ref_iter(&self) -> Self::Iter<'_>;
}

impl<T> RefIntoIterator for &T
where
    T: RefIntoIterator,
{
    type Item<'a>
        = T::Item<'a>
    where
        Self: 'a;

    type Iter<'a>
        = T::Iter<'a>
    where
        Self: 'a;

    #[inline]
    fn ref_iter(&self) -> Self::Iter<'_> {
        (*self).ref_iter()
    }
}
