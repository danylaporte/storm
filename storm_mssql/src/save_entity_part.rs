use crate::UpsertBuilder;
use storm::{CacheIsland, Entity};

pub trait SaveEntityPart: Entity {
    fn save_entity_part<'a>(&'a self, k: &'a Self::Key, builder: &mut UpsertBuilder<'a>);
}

impl<T> SaveEntityPart for CacheIsland<T>
where
    T: SaveEntityPart,
{
    fn save_entity_part<'a>(&'a self, k: &'a Self::Key, builder: &mut UpsertBuilder<'a>) {
        if let Some(v) = self.get() {
            v.save_entity_part(k, builder);
        }
    }
}

impl<T> SaveEntityPart for Option<T>
where
    T: SaveEntityPart,
{
    fn save_entity_part<'a>(&'a self, k: &'a Self::Key, builder: &mut UpsertBuilder<'a>) {
        if let Some(v) = self.as_ref() {
            v.save_entity_part(k, builder);
        }
    }
}
