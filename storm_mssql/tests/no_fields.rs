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
async fn no_fields() -> Result<()> {
    async_cell_lock::with_deadlock_check(async move {
        let ctx = create_ctx();
        let ctx = ctx.read().await?;
        let provider = ctx.provider().provide::<MssqlProvider>("").await?;

        provider
            .execute_with_args(
                "CREATE TABLE ##Tbl (Id INT NOT NULL);",
                &[],
                ExecuteArgs {
                    use_transaction: false,
                },
            )
            .await?;

        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction();
        let mut entities1 = trx.tbl_of::<Entity1>().await?;

        // insert
        entities1.insert(1, Entity1, &()).await?;

        assert!(entities1.get(&1).is_some());

        // update
        entities1.insert(1, Entity1, &()).await?;

        // insert
        entities1.insert(2, Entity1, &()).await?;

        // delete
        entities1.remove(2, &()).await?;

        let log = trx.commit().await?;

        let mut ctx = ctx.write().await?;

        ctx.apply_log(log);

        let ctx = ctx.read().await?;
        let entities1 = ctx.tbl_of::<Entity1>().await?;

        assert!(entities1.get(&1).is_some(),);
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
struct Entity1;

impl Entity for Entity1 {
    type Key = i32;
    type TrackCtx = ();
}
