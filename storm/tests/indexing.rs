use storm::{prelude::*, NoopLoad, Result};

fn create_ctx() -> QueueRwLock<Ctx> {
    QueueRwLock::new(Default::default())
}

#[tokio::test]
async fn create_async() -> Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.read().await;
    let _id: &usize = ctx.ref_as::<NextId>().await?;
    Ok(())
}

#[derive(Ctx, Default, NoopLoad)]
struct User {
    pub name: String,
}

impl Entity for User {
    type Key = usize;
}

#[indexing]
fn next_id(tbl: &Users) -> usize {
    tbl.iter().map(|t| t.0).max().unwrap_or_default()
}

#[indexing]
fn next_id2(_tbl: &Users, next_id: &NextId) -> usize {
    **next_id
}
