use async_cell_lock::AsyncOnceCell;
use async_trait::async_trait;
use storm::{
    Ctx, Entity, EntityDelete, EntityLoad, EntityUpsert, OptsTransaction, OptsVersion, Result,
    Version,
};
use vec_map::VecMap;

#[tokio::main]
async fn main() -> Result<()> {
    // creates a new context and passing required connections.
    let ctx = Ctx::new(ConnPool);

    // loading a table in memory.
    let users = ctx.users().await?;

    // try getting an entity from memory.
    if let Some(_) = users.get(&100) {
        println!("user found.");
    }

    println!("{}", users.contains_key(&2));

    // transform the context into a transaction.
    let mut transaction = ctx.transaction().await?;

    // get the table in memory (loading the table if not already in memory).
    let _users = transaction.users().await?;

    // get a mutable table in memory (loading the table if not already in memory).
    let mut users = transaction.users_mut().await?;

    // insert or update an entity.
    users
        .insert(
            32,
            User {
                name: "new user".into(),
            },
        )
        .await?;

    // delete and entity.
    users.remove(33).await?;

    // commit the transaction and get back a change log.
    let log = transaction.commit().await?;

    // open the context in mutation.
    let mut ctx = Ctx::new(ConnPool);

    // apply the change log.
    ctx.apply_log(log);

    Ok(())
}

struct ConnPool;

#[async_trait]
impl OptsTransaction for ConnPool {
    fn cancel(&self) {}

    async fn commit(&self) -> Result<()> {
        Ok(())
    }

    async fn transaction(&self) -> Result<()> {
        Ok(())
    }
}

impl OptsVersion for ConnPool {
    fn opts_version(&mut self) -> u64 {
        0
    }
}

#[derive(Ctx)]
struct Ctx {
    opts: ConnPool,
    //topic: Cache<usize, Topic>,
    users: AsyncOnceCell<Version<VecMap<usize, User>>>,
}

pub struct User {
    pub name: String,
}

impl Entity for User {
    type Key = usize;
}

#[async_trait]
impl EntityDelete<ConnPool> for User {
    async fn entity_delete(_key: &usize, _opts: &ConnPool) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl EntityLoad<ConnPool> for User {
    async fn entity_load(_opts: &ConnPool) -> Result<Vec<(usize, Self)>> {
        Ok(Vec::new())
    }
}

#[async_trait]
impl EntityUpsert<ConnPool> for User {
    async fn entity_upsert(&self, _key: &usize, _opts: &ConnPool) -> Result<()> {
        Ok(())
    }
}
