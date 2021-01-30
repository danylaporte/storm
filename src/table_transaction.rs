use crate::{Entity, Table, TableLog};

pub struct TableTransaction<'a, T: Table> {
    pub log: &'a TableLog<<T::Entity as Entity>::Row>,
    pub table: &'a T,
}

// Todo! implement the modification of the log.
