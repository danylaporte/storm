use crate::{Entity, TableTransaction};
use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
};
use vec_map::VecMap;

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

impl<'a, L, T: Table> Table for TableTransaction<'a, L, T> {
    type Entity = T::Entity;
}

impl<E> Table for VecMap<E::Key, E>
where
    E: Entity,
    E::Key: Clone + Into<usize>,
{
    type Entity = E;
}
