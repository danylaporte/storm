//use async_trait::async_trait;
use async_trait::async_trait;
use cache::Cache;
use storm::{
    mem::Transaction,
    provider::{LoadAll, Upsert},
    Ctx, Entity, OnceCell, Result,
};
use vec_map::VecMap;

#[tokio::main]
async fn main() -> Result<()> {
    let ctx = Ctx::default();
    let provider = &();
    //let users = ctx.users.get_or_load(provider).await?;

    let mut trx = ctx.transaction();
    let users = trx.users.get_mut(provider).await?;

    users
        .insert(
            2,
            User {
                name: "Test2".to_string(),
            },
            provider,
        )
        .await?;

    // creates a new context and passing required connections.

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

#[async_trait]
impl LoadAll<User> for () {
    async fn load_all<C: Default + Extend<(usize, User)>>(&self) -> Result<C> {
        Ok(C::default())
    }
}

#[async_trait]
impl Upsert<User> for () {
    async fn upsert(&self, _k: &usize, _v: &User) -> Result<()> {
        Ok(())
    }
}
