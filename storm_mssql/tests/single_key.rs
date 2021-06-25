use storm::{prelude::*, MssqlDelete, MssqlLoad, MssqlSave, Result};
use storm_mssql::{Execute, ExecuteArgs, MssqlFactory, MssqlProvider};
use tiberius::Config;

fn create_ctx() -> QueueRwLock<Ctx> {
    QueueRwLock::new(provider().into())
}

fn provider() -> ProviderContainer {
    let mut config = Config::default();
    config.database("master");
    #[cfg(target_os = "windows")]
    config.authentication(tiberius::AuthMethod::Integrated);
    config.trust_cert();

    let mut provider = ProviderContainer::new();
    provider.register("", MssqlFactory(config));

    provider
}

#[tokio::test]
async fn crud() -> Result<()> {
    async_cell_lock::with_deadlock_check(async move {
        let ctx = create_ctx();
        let ctx = ctx.read().await?;
        let provider = ctx.provider().provide::<MssqlProvider>("").await?;

        provider
            .execute_with_args(
                "CREATE TABLE ##Tbl (Id INT NOT NULL, Name NVARCHAR(100) NOT NULL, Other INT NULL);",
                &[],
                ExecuteArgs {
                    use_transaction: false,
                },
            )
            .await?;

        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction();
        let mut entities1 = trx.tbl_of::<Entity1>().await?;

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

        let e2 = Entity1 {
            name: "E2".to_string(),
            o: None,
        };
        entities1.insert(2, e2).await?;

        // delete
        entities1.remove(2).await?;

        let log = trx.commit().await?;

        let mut ctx = ctx.write().await?;

        ctx.apply_log(log);

        let ctx = ctx.read().await?;
        let entities1 = ctx.tbl_of::<Entity1>().await?;

        assert_eq!(
            entities1.get(&1).unwrap().clone(),
            Entity1 {
                name: "E1".to_string(),
                o: Some(5),
            }
        );

        assert!(entities1.get(&2).is_none());

        Ok(())
    })
    .await
}

#[derive(Clone, Ctx, Debug, MssqlDelete, MssqlLoad, MssqlSave, PartialEq)]
#[storm(
    table = "##Tbl",
    keys = "Id",
    collection = "hash_table",
    no_test = true
)]
struct Entity1 {
    name: String,

    #[storm(column = "Other")]
    o: Option<i32>,
}

impl Entity for Entity1 {
    type Key = i32;
}
