use crate::{Entity, Error, Row, Table, TableLog};
use std::convert::{TryFrom, TryInto};

pub trait ApplyLog {
    type Row: Row;
    fn apply_log(&mut self, log: TableLog<Self::Row>);
}

impl<T> ApplyLog for T
where
    T: Table,
    T::Entity: TryFrom<<T::Entity as Entity>::Row, Error = Error>,
    <T::Entity as Entity>::Key: From<<<T::Entity as Entity>::Row as Row>::Key>,
{
    type Row = <T::Entity as Entity>::Row;

    fn apply_log(&mut self, log: TableLog<Self::Row>) {
        log.remove.into_iter().for_each(|k| self.remove(&k.into()));

        log.add
            .into_iter()
            .for_each(|r| self.insert(r.key().into(), r.try_into().expect("Entity")));
    }
}
