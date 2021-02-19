pub trait Entity {
    type Key;
}

impl<T> Entity for Option<T>
where
    T: Entity,
{
    type Key = T::Key;
}
