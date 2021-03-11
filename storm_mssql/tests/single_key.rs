use std::collections::HashMap;
use storm::{
    mem::{Commit, Transaction},
    ApplyLog, AsyncOnceCell, Ctx, Entity, GetOrLoadAsync, MssqlLoad, MssqlSave, Result,
};
use storm_mssql::{Execute, MssqlProvider};
use tiberius::{AuthMethod, Config};

fn provider() -> MssqlProvider {
    let mut config = Config::default();
    config.database("master");
    config.authentication(AuthMethod::Integrated);
    config.trust_cert();

    MssqlProvider::new(config)
}

#[tokio::test]
async fn crud() -> Result<()> {
    let provider = &provider();
    let transaction = provider.transaction().await?;

    transaction
        .execute(
            "CREATE TABLE ##Tbl (Id INT NOT NULL, Name NVARCHAR(100) NOT NULL, Other INT NULL);",
            &[],
        )
        .await?;

    let ctx = TestCtx::default();
    let mut trx = ctx.transaction();
    let entities1 = trx.entities1.get_mut_or_init(&transaction).await?;

    let e1 = Entity1 {
        name: "E1".to_string(),
        o: None,
    };

    // insert
    entities1.insert(1, e1, &transaction).await?;

    let mut e1 = entities1.get(&1).unwrap().clone();

    e1.o = Some(5);

    // update
    entities1.insert(1, e1, &transaction).await?;

    transaction.commit().await?;
    let log = trx.commit();

    let mut ctx = TestCtx::default();
    ctx.apply_log(log);

    let ctx = TestCtx::default();

    let entities1 = ctx.entities1.get_or_load_async(provider).await?;

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
struct TestCtx {
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
