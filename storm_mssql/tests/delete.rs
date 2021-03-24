use storm::{Entity, MssqlDelete};

#[derive(MssqlDelete)]
#[storm(table = "t", keys = "id")]
pub struct EntityWithDuplicateKey {
    pub name: String,
    pub id: i32,
}

impl Entity for EntityWithDuplicateKey {
    type Key = i32;
}
