use async_cell_lock::AsyncOnceCell;
use async_trait::async_trait;
use std::convert::TryFrom;
use storm::{Ctx, Entity, Error, Result, Row, RowLoad};
use vec_map::VecMap;

#[tokio::main]
async fn main() -> Result<()> {
    let c: Ctx = Ctx::new(ConnPool);
    let users = c.users().await?;

    if let Some(_) = users.get(&100) {
        println!("user found.");
    }

    println!("{}", users.contains_key(&2));

    let t = c.transaction();
    let _users = t.users().await?;

    let log = t.commit().await?;

    let mut c: Ctx = Ctx::new(ConnPool);

    c.apply_log(log);

    Ok(())
}

struct ConnPool;

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
