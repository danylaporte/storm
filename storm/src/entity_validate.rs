use crate::Error;

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
