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
            let _id: &usize = ctx.ref_as::<NextId>().await?;
            Ok(())
        },
        "create_async",
    )
    .await
}

#[derive(Ctx, Default, NoopLoad)]
struct User {
    #[allow(dead_code)]
    pub name: String,
}

impl Entity for User {
    type Key = usize;
    type TrackCtx = ();
}

#[indexing]
fn next_id(tbl: &Users) -> usize {
    tbl.iter().map(|t| *t.0).max().unwrap_or_default()
}

#[indexing]
fn next_id2(_tbl: &Users, next_id: &NextId) -> usize {
    **next_id
}

#[indexing]
fn index_with_ctx(_ctx: &Ctx, tbl: &Users) -> usize {
    tbl.len()
}
