use crate::{Entity, Table, TableGet, TableLog};

pub struct TableTransaction<'a, L, T> {
    pub log: L,
    pub table: &'a T,
}

impl<'a, L, T> TableTransaction<'a, L, T> {
    pub fn get(&self, k: &<T::Entity as Entity>::Key) -> Option<&T::Entity>
    where
        <T::Entity as Entity>::Key: PartialEq,
        L: AsRef<TableLog<T>>,
        T: TableGet,
    {
        let log = self.log.as_ref();

        if let Some(v) = log.add_get(k) {
            return Some(v);
        }

        self.table.get(k).filter(|_| !log.is_removed(k))
    }
}

impl<'a, T: Table> TableTransaction<'a, &'a mut TableLog<T>, T> {
    pub fn insert(&mut self, k: <T::Entity as Entity>::Key, v: T::Entity)
    where
        <T::Entity as Entity>::Key: PartialEq,
    {
        self.log.insert(k, v);
    }

    pub fn remove(&mut self, k: <T::Entity as Entity>::Key)
    where
        <T::Entity as Entity>::Key: PartialEq,
    {
        self.log.remove(k);
    }
}
