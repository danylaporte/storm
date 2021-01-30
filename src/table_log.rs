use crate::Row;

pub struct TableLog<R: Row> {
    pub(crate) add: Vec<R>,
    pub(crate) remove: Vec<R::Key>,
}

impl<R: Row> Default for TableLog<R> {
    fn default() -> Self {
        Self {
            add: Vec::new(),
            remove: Vec::new(),
        }
    }
}

// TODO! Ajouter la logique de modification.
