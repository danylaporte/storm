use std::future::Future;

use crate::{entity::EntityTrx, Error, ObjTrxBase, Result, Trx};

pub trait EntityValidate {
    fn entity_validate(&self, error: &mut Option<Error>);
}

#[cfg(feature = "cache")]
impl<T> EntityValidate for cache::CacheIsland<T>
where
    T: EntityValidate,
{
    fn entity_validate(&self, error: &mut Option<Error>) {
        if let Some(v) = self.get() {
            v.entity_validate(error);
        }
    }
}

impl<T: EntityValidate> EntityValidate for Option<T> {
    fn entity_validate(&self, error: &mut Option<Error>) {
        if let Some(v) = self {
            v.entity_validate(error);
        }
    }
}

#[allow(clippy::manual_async_fn)]
pub(crate) fn validate_on_change<'a, 'b, E>(
    trx: &'b mut Trx<'a>,
    key: &'b E::Key,
    entity: &'b mut E,
    track: &'b E::TrackCtx,
) -> impl Future<Output = Result<()>> + Send + use<'a, 'b, E>
where
    E: EntityTrx + EntityValidate,
    E::Tbl: ObjTrxBase,
{
    async move {
        let mut error = None;

        if let Err(e) = E::change().call(trx, key, entity, track).await {
            error = Some(e);
        }

        EntityValidate::entity_validate(&*entity, &mut error);

        match error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}
