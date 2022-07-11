use std::ops::Deref;

use crate::{Entity, HashTable, VecTable};

pub trait Length {
    fn len(&self) -> usize;
}

impl<E> Length for VecTable<E>
where
    E: Entity,
{
    fn len(&self) -> usize {
        self.deref().len()
    }
}

impl<E> Length for HashTable<E>
where
    E: Entity,
{
    fn len(&self) -> usize {
        self.deref().len()
    }
}
