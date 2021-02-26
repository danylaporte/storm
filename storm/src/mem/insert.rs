use crate::Entity;

pub trait Insert<E: Entity> {
    fn insert(&mut self, k: E::Key, v: E);
}

impl<E, T> Insert<E> for &mut T
where
    E: Entity,
    T: Insert<E>,
{
    fn insert(&mut self, k: E::Key, v: E) {
        (**self).insert(k, v)
    }
}
