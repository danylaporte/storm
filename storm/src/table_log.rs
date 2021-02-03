use crate::{Entity, Table, TableAppyLog};

pub struct TableLog<T: Table> {
    pub(crate) add: Vec<(<T::Entity as Entity>::Key, T::Entity)>,
    pub(crate) remove: Vec<<T::Entity as Entity>::Key>,
}

impl<T: Table> TableLog<T>
where
    <T::Entity as Entity>::Key: PartialEq,
{
    pub(crate) fn add_get(&self, k: &<T::Entity as Entity>::Key) -> Option<&T::Entity> {
        self.add.iter().find(|t| &t.0 == k).map(|t| &t.1)
    }

    fn add_index_of(&self, k: &<T::Entity as Entity>::Key) -> Option<usize> {
        self.add.iter().position(|t| &t.0 == k)
    }

    pub(crate) fn apply_log(self, table: &mut T, version: u64)
    where
        T: TableAppyLog,
    {
        self.remove.iter().for_each(|k| table.remove(k, version));
        self.add
            .into_iter()
            .for_each(|t| table.insert(t.0, t.1, version));
    }

    pub(crate) fn insert(&mut self, k: <T::Entity as Entity>::Key, v: T::Entity) {
        match self.add_index_of(&k) {
            Some(index) => self.add[index] = (k, v),
            None => {
                if let Some(index) = self.remove.iter().position(|t| t == &k) {
                    self.remove.swap_remove(index);
                }

                self.add.push((k, v));
            }
        }
    }

    pub(crate) fn is_removed(&self, k: &<T::Entity as Entity>::Key) -> bool {
        self.remove.contains(k)
    }

    pub(crate) fn remove(&mut self, k: <T::Entity as Entity>::Key) {
        match self.add_index_of(&k) {
            Some(index) => {
                self.add.swap_remove(index);
                self.remove.push(k);
            }
            None => {
                if !self.remove.contains(&k) {
                    self.remove.push(k);
                }
            }
        }
    }
}

impl<T: Table> Default for TableLog<T> {
    fn default() -> Self {
        Self {
            add: Vec::new(),
            remove: Vec::new(),
        }
    }
}
