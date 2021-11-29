use storm::{Ctx, Entity, MssqlLoad, MssqlSave};

#[derive(Ctx, MssqlLoad, MssqlSave)]
#[storm(table = "t", keys = "id", collection = "hash_table", no_test = true)]
pub struct EntityWithDuplicateKey {
    pub name: String,
    pub id: i32,
}

impl Entity for EntityWithDuplicateKey {
    type Key = i32;
    type TrackCtx = ();
}

#[derive(Ctx, MssqlLoad, MssqlSave)]
#[storm(table = "t", keys = "id", collection = "hash_table", no_test = true)]
pub struct EntitySaveWith {
    #[storm(save_with = "buffer_save_with")]
    pub buffer: String,
}

impl Entity for EntitySaveWith {
    type Key = i32;
    type TrackCtx = ();
}

fn buffer_save_with(_key: &i32, value: &EntitySaveWith) -> Vec<u8> {
    value.buffer.as_bytes().to_vec()
}

#[derive(Ctx, MssqlLoad, MssqlSave)]
#[storm(table = "t", keys = "id", collection = "hash_table", no_test = true)]
pub struct EntityWithPart {
    #[storm(part = true)]
    part: Option<EntityPart>,
}

impl Entity for EntityWithPart {
    type Key = i32;
    type TrackCtx = ();
}

#[derive(MssqlLoad, MssqlSave)]
#[storm(table = "t", keys = "id", no_test = true, part = true)]
pub struct EntityPart {
    pub i: i32,
}

impl Entity for EntityPart {
    type Key = i32;
    type TrackCtx = ();
}
