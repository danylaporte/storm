use crate::Entity;

pub trait Remove<E: Entity> {
    fn remove(&mut self, k: E::Key);
}

impl<E, T> Remove<E> for &mut T
where
    E: Entity,
    T: Remove<E>,
{
    fn remove(&mut self, k: E::Key) {
        (**self).remove(k)
    }
}
