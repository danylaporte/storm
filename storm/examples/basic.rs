use cache::CacheIsland;
use storm::{prelude::*, NoopDelete, NoopLoad, NoopSave, Result};

#[tokio::main]
async fn main() -> Result<()> {
    async_cell_lock::with_deadlock_check(async move {
        let lock = QueueRwLock::new(Ctx::default());

        let ctx = lock.read().await?;
        let _topics = ctx.tbl_of::<Topic>().await?;
        let _users = ctx.tbl_of::<User>().await?;

        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction();
        let mut users = trx.tbl_of::<User>().await?;

        users
            .insert(
                1,
                User {
                    name: "Test2".to_string(),
                },
            )
            .await?;

        users.remove(1).await?;

        let _topic = trx.tbl_of::<Topic>().await?;
        let log = trx.commit().await?;

        let mut ctx = ctx.write().await?;
        ctx.apply_log(log);

        Ok(())
    })
    .await
}

#[derive(NoopDelete, NoopLoad, NoopSave, Ctx)]
struct Topic {
    pub title: String,
    pub comment: CacheIsland<String>,
}

impl Entity for Topic {
    type Key = usize;
}

#[derive(NoopDelete, NoopLoad, NoopSave, Ctx)]
struct User {
    pub name: String,
}

impl Entity for User {
    type Key = usize;
}
