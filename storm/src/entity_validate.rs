use crate::{EntityAsset, Error, Result, Trx};

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

pub(crate) async fn validate_on_change<'a, E>(
    trx: &mut Trx<'a>,
    key: &E::Key,
    entity: &mut E,
    track: &E::TrackCtx,
) -> Result<()>
where
    E: EntityAsset + EntityValidate,
{
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
