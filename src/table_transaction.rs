use crate::{Entity, EntityDelete, EntityUpsert, Result, Table, TableGet, TableLog};

pub struct TableTransaction<'a, L, O, T> {
    pub log: L,
    pub opts: &'a O,
    pub table: &'a T,
}

impl<'a, L, O, T> TableTransaction<'a, L, O, T> {
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

impl<'a, O, T: Table> TableTransaction<'a, &'a mut TableLog<T>, O, T> {
    pub async fn insert(&mut self, k: <T::Entity as Entity>::Key, v: T::Entity) -> Result<()>
    where
        <T::Entity as Entity>::Key: PartialEq,
        T::Entity: EntityUpsert<O>,
    {
        v.entity_upsert(&k, self.opts).await?;
        self.log.insert(k, v);
        Ok(())
    }

    pub async fn remove(&mut self, k: <T::Entity as Entity>::Key) -> Result<()>
    where
        <T::Entity as Entity>::Key: PartialEq,
        T::Entity: EntityDelete<O>,
    {
        T::Entity::entity_delete(&k, self.opts).await?;
        self.log.remove(k);
        Ok(())
    }
}

impl<'a, L, O, T: Table> Table for TableTransaction<'a, L, O, T> {
    type Entity = T::Entity;
}

impl<'a, L, O, T> TableGet for TableTransaction<'a, L, O, T>
where
    <T::Entity as Entity>::Key: PartialEq,
    L: AsRef<TableLog<T>>,
    T: TableGet,
{
    fn get(&self, k: &<Self::Entity as Entity>::Key) -> Option<&Self::Entity> {
        TableTransaction::get(self, k)
    }
}
