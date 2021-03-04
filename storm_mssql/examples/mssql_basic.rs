use std::usize;
use storm::{
    mem::{Commit, Transaction},
    ApplyLog, Ctx, Entity, GetOrLoadAsync, OnceCell, Result,
};
use storm_mssql::ClientFactory;
use tiberius::{FromSql, ToSql};
use vec_map::VecMap;

#[tokio::main]
async fn main() -> Result<()> {
    let mut config = tiberius::Config::default();
    config.database("master");

    let provider = &config.create_provider();
    let ctx = Ctx::default();

    let _users = ctx.users.get_or_load_async(provider).await?;

    let mssql_trx = provider.transaction().await?;
    let mut trx = ctx.transaction();

    let users = trx.users.get_mut_or_init(provider).await?;

    users
        .insert(
            UserId(2),
            User {
                name: "Test2".to_string(),
            },
            &mssql_trx,
        )
        .await?;

    mssql_trx.commit().await?;

    let log = trx.commit();

    let mut ctx = Ctx::default();

    ctx.apply_log(log);

    // creates a new context and passing required connections.

    Ok(())
}

#[derive(Ctx, Default)]
struct Ctx {
    users: OnceCell<VecMap<UserId, User>>,
}

#[derive(storm::MssqlLoad, storm::MssqlSave)]
#[storm(table = "[dbo].[users]", keys = "id")]
struct User {
    pub name: String,
}

impl Entity for User {
    type Key = UserId;
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct UserId(i32);

impl ToSql for UserId {
    fn to_sql(&self) -> tiberius::ColumnData<'_> {
        self.0.to_sql()
    }
}

impl From<UserId> for usize {
    fn from(v: UserId) -> Self {
        v.0 as usize
    }
}

impl<'a> FromSql<'a> for UserId {
    fn from_sql(value: &'a tiberius::ColumnData<'static>) -> tiberius::Result<Option<Self>> {
        match i32::from_sql(value)? {
            Some(v) => Ok(Some(Self(v))),
            None => Ok(None),
        }
    }
}

impl<'a> storm_mssql::FromSql<'a> for UserId {
    type Column = i32;

    fn from_sql(col: Option<Self::Column>) -> Result<Self> {
        match col {
            Some(v) => Ok(UserId(v)),
            None => Err(storm::Error::ColumnNull),
        }
    }
}
