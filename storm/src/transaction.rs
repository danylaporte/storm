use crate::{Entity, MapTransaction};
use std::collections::HashMap;

pub trait Transaction<'a> {
    type Transaction: Send;

    #[must_use]
    fn transaction(&'a self) -> Self::Transaction;
}

#[cfg(feature = "cache")]
impl<'a, E: Entity, S> Transaction<'a> for cache::Cache<E::Key, E, S>
where
    E: 'a,
    S: Sync + 'a,
{
    type Transaction = MapTransaction<E, &'a Self>;

    fn transaction(&'a self) -> Self::Transaction {
        MapTransaction::new(self)
    }
}

impl<'a, E: Entity, S> Transaction<'a> for HashMap<E::Key, E, S>
where
    E: 'a,
    S: Sync + 'a,
{
    type Transaction = MapTransaction<E, &'a Self>;

    fn transaction(&'a self) -> Self::Transaction {
        MapTransaction::new(self)
    }
}

#[cfg(feature = "vec-map")]
impl<'a, E: Entity> Transaction<'a> for vec_map::VecMap<E::Key, E>
where
    E: 'a,
{
    type Transaction = MapTransaction<E, &'a Self>;

    fn transaction(&'a self) -> Self::Transaction {
        MapTransaction::new(self)
    }
}
