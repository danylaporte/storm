use async_cell_lock::QueueRwLock;
use cache::Cache;
use storm::{ApplyLog, Commit, Ctx, CtxProvider, Entity, OnceCell, Result, Transaction};
use vec_map::VecMap;

#[tokio::main]
async fn main() -> Result<()> {
    let lock = QueueRwLock::new(CtxProvider {
        ctx: Ctx::default(),
        provider: (),
    });

    let ctx_provider = lock.read().await;

    let _topic = ctx_provider.topic();
    let _users = ctx_provider.users().await?;

    let ctx_provider = ctx_provider.queue().await;

    let mut trx = ctx_provider.transaction().await?;

    let _users = trx.users().await?;
    let _users_mut = trx.users_mut().await?;

    // not working yet:
    // users_mut.insert(
    //     1,
    //     User {
    //         name: "Test2".to_string(),
    //     },
    // );

    let _topic = trx.topic();
    let _topic_mut = trx.topic_mut();

    let log = trx.commit().await?;

    let mut ctx_provider = ctx_provider.write().await;

    ctx_provider.apply_log(log);

    Ok(())
}

#[derive(Ctx, Default)]
struct Ctx {
    topic: Cache<usize, User>,
    users: OnceCell<VecMap<usize, User>>,
}

struct User {
    pub name: String,
}

impl Entity for User {
    type Key = usize;
}
