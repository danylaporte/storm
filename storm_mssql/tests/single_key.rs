use std::collections::HashMap;
use storm::{
    prelude::*, provider::ProviderContainer, AsyncOnceCell, Connected, Ctx, Entity, MssqlLoad,
    MssqlSave, QueueRwLock, Result,
};
use storm_mssql::{Execute, MssqlFactory, MssqlProvider};
use tiberius::{AuthMethod, Config};
fn create_ctx() -> QueueRwLock<Connected<Ctx>> {
    QueueRwLock::new(Connected {
        ctx: Ctx::default(),
        provider: provider(),
    })
}
fn provider() -> ProviderContainer {
    let mut config = Config::default();
    config.database("master");
    config.authentication(AuthMethod::Integrated);
    config.trust_cert();

    let mut provider = ProviderContainer::new();
    provider.register("", MssqlFactory(config));

    provider
}

#[tokio::test]
async fn crud() -> Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.read().await;
    let provider = ctx.provider.provide::<MssqlProvider>("").await?;

    provider
        .execute(
            "CREATE TABLE ##Tbl (Id INT NOT NULL, Name NVARCHAR(100) NOT NULL, Other INT NULL);",
            &[],
        )
        .await?;

    let ctx = ctx.queue().await;
    let mut trx = ctx.transaction();

    let mut entities1 = trx.entities1_mut().await?;

    let e1 = Entity1 {
        name: "E1".to_string(),
        o: None,
    };

    // insert
    entities1.insert(1, e1).await?;

    let mut e1 = entities1.get(&1).unwrap().clone();

    e1.o = Some(5);

    // update
    entities1.insert(1, e1).await?;

    let log = trx.commit().await?;

    let mut ctx = ctx.write().await;

    ctx.apply_log(log);

    let ctx = ctx.read().await;
    let entities1 = ctx.entities1().await?;

    assert_eq!(
        entities1.get(&1).unwrap().clone(),
        Entity1 {
            name: "E1".to_string(),
            o: Some(5),
        }
    );

    Ok(())
}

#[derive(Ctx, Default)]
struct Ctx {
    entities1: AsyncOnceCell<HashMap<i32, Entity1>>,
}

#[derive(Clone, Debug, MssqlLoad, MssqlSave, PartialEq)]
#[storm(table = "##Tbl", keys = "Id")]
struct Entity1 {
    name: String,

    #[storm(column = "Other")]
    o: Option<i32>,
}

impl Entity for Entity1 {
    type Key = i32;
}
