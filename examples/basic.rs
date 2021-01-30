use async_cell_lock::AsyncOnceCell;
use async_trait::async_trait;
use std::convert::TryFrom;
use storm::{Ctx, Entity, Error, Load, Result, Row};
use vec_map::VecMap;

#[tokio::main]
async fn main() -> Result<()> {
    let c: Ctx = todo!();
    let users = c.users().await?;

    let t = c.transaction();
    let users = t.users().await?;

    let log = t.commit().await?;

    let mut c: Ctx = todo!();

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
    name: String,
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
impl Load<ConnPool> for UserDb {
    async fn load(opts: &ConnPool) -> Result<Vec<Self>> {
        Ok(Vec::new())
    }
}
