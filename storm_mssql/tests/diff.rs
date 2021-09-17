use std::future::ready;

use storm::{prelude::*, MssqlLoad, MssqlSave, Result};
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

#[derive(Ctx, MssqlLoad, MssqlSave)]
#[storm(
    table = "##Tbl",
    keys = "id",
    diff = true,
    collection = "hash_table",
    no_test = true
)]
pub struct Entity1 {
    pub v: i32,
}

impl Entity for Entity1 {
    type Key = i32;
    type TrackCtx = ();

    fn track_insert<'a>(
        &'a self,
        _key: &'a Self::Key,
        old: Option<&'a Self>,
        _ctx: &'a mut storm::CtxTransaction,
        _tracker: &'a Self::TrackCtx,
    ) -> storm::BoxFuture<'a, Result<()>> {
        println!("current: {}, old: {:?}", self.v, old.map(|e| e.v));
        Box::pin(ready(Ok(())))
    }
}

#[tokio::test]
async fn diff_insert() -> Result<()> {
    async_cell_lock::with_deadlock_check(async move {
        let lock = create_ctx();
        let ctx = lock.read().await?;
        let provider = ctx.provider().provide::<MssqlProvider>("").await?;

        provider
            .execute_with_args(
                "CREATE TABLE ##Tbl (Id INT NOT NULL, V Int NOT NULL);",
                &[],
                ExecuteArgs {
                    use_transaction: false,
                },
            )
            .await?;

        let ctx = ctx.queue().await?;
        let mut trx = ctx.transaction();
        let mut entities1 = trx.tbl_of::<Entity1>().await?;

        let e1 = Entity1 { v: 1 };

        // insert
        entities1.insert(1, e1, &()).await?;

        let log = trx.commit().await?;
        ctx.write().await?.apply_log(log);

        let ctx = lock.queue().await?;

        let mut trx = ctx.transaction();
        let mut entities1 = trx.tbl_of::<Entity1>().await?;

        let e1 = Entity1 { v: 2 };

        entities1.insert(1, e1, &()).await?;

        Ok(())
    })
    .await
}
