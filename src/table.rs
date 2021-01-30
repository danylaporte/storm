use crate::Entity;
use std::{
    collections::HashMap,
    hash::{BuildHasher, Hash},
};
use vec_map::VecMap;

pub trait Table: Sized {
    type Entity: Entity;

    fn insert(&mut self, k: <Self::Entity as Entity>::Key, v: Self::Entity);

    fn remove(&mut self, k: &<Self::Entity as Entity>::Key);
}

impl<E, S> Table for HashMap<E::Key, E, S>
where
    E: Entity,
    E::Key: Eq + Hash,
    S: BuildHasher,
{
    type Entity = E;

    fn insert(&mut self, k: E::Key, v: E) {
        HashMap::insert(self, k, v);
    }

    fn remove(&mut self, k: &E::Key) {
        HashMap::remove(self, k);
    }
}

impl<E> Table for VecMap<E::Key, E>
where
    E: Entity,
    E::Key: Clone + Into<usize>,
{
    type Entity = E;

    fn insert(&mut self, k: E::Key, v: E) {
        VecMap::insert(self, k, v);
    }

    fn remove(&mut self, k: &E::Key) {
        VecMap::remove(self, k);
    }
}
