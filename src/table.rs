use crate::Entity;
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

#[cfg(feature = "vec-map")]
impl<E> Table for vec_map::VecMap<E::Key, E>
where
    E: Entity,
    E::Key: Clone + Into<usize>,
{
    type Entity = E;
}
