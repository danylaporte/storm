use async_cell_lock::QueueRwLock;
use storm::{indexing, prelude::*, AsyncOnceCell, Connected, Ctx, Entity, NoopLoad, Result};
use vec_map::VecMap;

fn create_ctx() -> QueueRwLock<Connected<Ctx>> {
    QueueRwLock::new(Connected {
        ctx: Ctx::default(),
        provider: Default::default(),
    })
}

#[tokio::test]
async fn create_async() -> Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.read().await;
    let _id: &usize = ctx.next_id().await?;
    Ok(())
}

#[derive(Ctx, Default)]
struct Ctx {
    tbl: AsyncOnceCell<Version<VecMap<usize, User>>>,

    #[storm(index = true)]
    next_id: AsyncOnceCell<NextId>,
}

#[derive(Default, NoopLoad)]
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
