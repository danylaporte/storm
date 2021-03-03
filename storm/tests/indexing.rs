use std::usize;
use storm::{indexing, prelude::*, Ctx, Entity, OnceCell};
use vec_map::VecMap;

#[derive(Ctx, Default)]
struct Ctx {
    tbl: OnceCell<Version<VecMap<usize, User>>>,
    #[storm(index = true)]
    next_id: OnceCell<NextId>,
}

#[derive(Default)]
struct User {
    pub name: String,
}

impl Entity for User {
    type Key = usize;
}

#[indexing]
fn next_id(tbl: &Tbl) -> usize {
    tbl.iter().map(|t| t.0).max().unwrap_or_default()
}

#[indexing]
fn next_id2(_tbl: &Tbl, next_id: &NextId) -> usize {
    **next_id
}
