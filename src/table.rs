use crate::{Entity, TableTransaction};
use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
};
use vec_map::VecMap;

pub trait Table: Sized {
    type Entity: Entity;
}

impl<T> Table for &T
where
    T: Table,
{
    type Entity = T::Entity;
}

impl<T> Table for &mut T
where
    T: Table,
{
    type Entity = T::Entity;
}

impl<E, S> Table for HashMap<E::Key, E, S>
where
    E: Entity,
    E::Key: Eq + Hash,
    S: BuildHasher,
{
    type Entity = E;
}

impl<'a, T: Table> Table for TableTransaction<'a, T> {
    type Entity = T::Entity;
}

impl<E> Table for VecMap<E::Key, E>
where
    E: Entity,
    E::Key: Clone + Into<usize>,
{
    type Entity = E;
}
