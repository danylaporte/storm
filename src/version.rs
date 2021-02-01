use crate::{Entity, Table, TableAppyLog};
use std::{
    iter::FromIterator,
    ops::{Deref, DerefMut},
};

pub struct Version<T> {
    table: T,
    version: u64,
}

impl<T> Deref for Version<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.table
    }
}

impl<T> DerefMut for Version<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.table
    }
}

impl<E, U> FromIterator<E> for Version<U>
where
    U: FromIterator<E>,
{
    fn from_iter<T: IntoIterator<Item = E>>(iter: T) -> Self {
        Version {
            table: U::from_iter(iter),
            version: 0,
        }
    }
}

impl<T> Table for Version<T>
where
    T: Table,
{
    type Entity = T::Entity;
}

impl<T> TableAppyLog for Version<T>
where
    T: TableAppyLog,
{
    fn insert(&mut self, k: <Self::Entity as Entity>::Key, v: Self::Entity, version: u64) {
        self.version = version;
        T::insert(self, k, v, version);
    }

    fn remove(&mut self, k: &<Self::Entity as Entity>::Key, version: u64) {
        self.version = version;
        T::remove(self, k, version);
    }
}
