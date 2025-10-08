#![allow(clippy::unwrap_used)]

use storm::{Ctx, Entity, MssqlSave};

#[derive(Ctx, MssqlSave, PartialEq)]
#[storm(table = "t", keys = "id")]
pub struct EntityWithDuplicateKey {
    pub name: String,
    pub id: u32,
}

impl Entity for EntityWithDuplicateKey {
    type Key = u32;
}

#[derive(Ctx, MssqlSave, PartialEq)]
#[storm(table = "t", keys = "id")]
pub struct EntitySaveWith {
    #[storm(save_with = "buffer_save_with")]
    pub buffer: String,
}

impl Entity for EntitySaveWith {
    type Key = u32;
}

fn buffer_save_with(_key: &u32, value: &EntitySaveWith) -> Vec<u8> {
    value.buffer.as_bytes().to_vec()
}

// #[derive(Ctx, MssqlSave, PartialEq)]
// #[storm(table = "t", keys = "id")]
// pub struct EntityWithPart {
//     #[storm(part = true)]
//     part: Option<EntityPart>,
// }

// impl Entity for EntityWithPart {
//     type Key = u32;
// }

// #[derive(MssqlSave, PartialEq)]
// #[storm(table = "t", keys = "id", no_test = true)]
// pub struct EntityPart {
//     pub i: u32,
// }

// impl Entity for EntityPart {
//     type Key = u32;
// }

// impl Gc for EntityPart {}
