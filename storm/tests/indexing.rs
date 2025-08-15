use storm::{prelude::*, NoopLoad, Result};

fn create_ctx() -> QueueRwLock<Ctx> {
    QueueRwLock::new(Default::default(), "ctx")
}

#[tokio::test]
async fn create_async() -> Result<()> {
    async_cell_lock::with_deadlock_check(
        async move {
            let ctx = create_ctx();
            let ctx = ctx.read().await?;
            let _id: &u32 = ctx.ref_as::<NextId>().await?;
            Ok(())
        },
        "create_async",
    )
    .await
}

#[derive(Ctx, Default, NoopLoad, PartialEq)]
struct User {
    #[allow(dead_code)]
    pub name: String,
}

impl Entity for User {
    type Key = u32;
}

#[indexing]
fn next_id(tbl: &Users) -> u32 {
    tbl.iter().map(|t| *t.0).max().unwrap_or_default()
}

#[indexing]
fn next_id2(_tbl: &Users, next_id: &NextId) -> u32 {
    **next_id
}

#[indexing]
fn index_with_ctx(_ctx: &Ctx, tbl: &Users) -> u32 {
    tbl.len() as u32
}
