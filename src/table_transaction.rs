use crate::{Entity, Table, TableGet, TableLog};

pub struct TableTransaction<'a, T: Table> {
    pub log: &'a TableLog<T>,
    pub table: &'a T,
}

impl<'a, T: Table> TableTransaction<'a, T> {
    pub fn get(&self, k: &<T::Entity as Entity>::Key) -> Option<&T::Entity>
    where
        T: TableGet,
        <T::Entity as Entity>::Key: PartialEq,
    {
        if let Some(v) = self.log.add_get(k) {
            return Some(v);
        }

        self.table.get(k).filter(|_| !self.log.is_removed(k))
    }
}
