use crate::{Entity, TableTransaction};
use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
};

pub trait Table: Sized {
    type Entity: Entity;
}

impl<E, S> Table for HashMap<E::Key, E, S>
where
    E: Entity,
    E::Key: Eq + Hash,
    S: BuildHasher,
{
    type Entity = E;
}

impl<'a, L, O, T: Table> Table for TableTransaction<'a, L, O, T> {
    type Entity = T::Entity;
}

#[cfg(feature = "vec-map")]
impl<E> Table for vec_map::VecMap<E::Key, E>
where
    E: Entity,
    E::Key: Clone + Into<usize>,
{
    type Entity = E;
}
