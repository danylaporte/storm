use crate::{BoxFuture, Entity, Result};

pub trait Upsert<E: Entity>: Send + Sync {
    fn upsert<'a>(&'a self, k: &'a E::Key, v: &'a E) -> BoxFuture<'a, Result<()>>;
}

impl<E, PROVIDER> Upsert<E> for &PROVIDER
where
    E: Entity + Sync,
    E::Key: Sync,
    PROVIDER: Upsert<E> + Send + Sync,
{
    fn upsert<'a>(&'a self, k: &'a E::Key, v: &'a E) -> BoxFuture<'a, Result<()>> {
        (**self).upsert(k, v)
    }
}

/// This trait is implemented when the entity or the key must be changed while insert or update is performed.
/// This case appears when there are triggers on the table, a sequence / identity column.
///
/// [UpsertMut](UpsertMut) trait is not the same as the [Upsert](Upsert) trait since the former need to take
/// a key and entity as mutable parameters.
pub trait UpsertMut<E: Entity>: Send + Sync {
    fn upsert_mut<'a>(&'a self, k: &'a mut E::Key, v: &'a mut E) -> BoxFuture<'a, Result<()>>;
}

impl<E, PROVIDER> UpsertMut<E> for &PROVIDER
where
    E: Entity + Sync,
    E::Key: Sync,
    PROVIDER: UpsertMut<E> + Send + Sync,
{
    fn upsert_mut<'a>(&'a self, k: &'a mut E::Key, v: &'a mut E) -> BoxFuture<'a, Result<()>> {
        (**self).upsert_mut(k, v)
    }
}
