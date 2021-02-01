use async_cell_lock::AsyncOnceCell;
use async_trait::async_trait;
use std::convert::TryFrom;
use storm::{Ctx, Entity, Error, OptsTransaction, Result, Row, RowLoad};
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
    users.insert(
        32,
        User {
            name: "new user".into(),
        },
    );

    // delete and entity.
    users.remove(33);

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

#[derive(Ctx)]
struct Ctx {
    users: AsyncOnceCell<VecMap<usize, User>>,
    opts: ConnPool,
}

pub struct User {
    pub name: String,
}

impl Entity for User {
    type Key = usize;
    type Row = UserDb;
}

pub struct UserDb {
    id: usize,
    name: String,
}

impl Row for UserDb {
    type Key = usize;

    fn key(&self) -> Self::Key {
        self.id
    }
}

impl TryFrom<UserDb> for User {
    type Error = Error;

    fn try_from(db: UserDb) -> Result<Self> {
        Ok(Self { name: db.name })
    }
}

#[async_trait]
impl RowLoad<ConnPool> for UserDb {
    async fn row_load(_opts: &ConnPool) -> Result<Vec<Self>> {
        Ok(Vec::new())
    }
}
