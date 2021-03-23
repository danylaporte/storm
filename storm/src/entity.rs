pub trait Entity: Send + Sync + 'static {
    type Key: Send + Sync;
}

#[cfg(feature = "cache")]
impl<T> Entity for cache::CacheIsland<T>
where
    T: Entity,
{
    type Key = T::Key;
}

impl<T> Entity for Option<T>
where
    T: Entity,
{
    type Key = T::Key;
}
