#![allow(clippy::unwrap_used)]

use storm::{
    MssqlDelete, MssqlLoad, Result,
    prelude::*,
    provider::{Delete, LoadOne},
};
use storm_mssql::{Execute, ExecuteArgs, MssqlFactory, MssqlProvider};
use tiberius::Config;

fn create_ctx() -> QueueRwLock<Ctx> {
    QueueRwLock::new(provider().into(), "ctx")
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

#[async_cell_lock::tokio_test]
async fn load_by_composite_key() -> Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.read().await?;
    let provider = ctx.provider().provide::<MssqlProvider>("").await?;

    provider
        .execute_with_args(
            "CREATE TABLE ##CompositeTbl (A INT NOT NULL, B INT NOT NULL, Name NVARCHAR(100) NOT NULL);
            INSERT INTO ##CompositeTbl (A, B, Name) VALUES (1, 1, 'A1B1'), (1, 2, 'A1B2'), (2, 1, 'A2B1');",
            &[],
            ExecuteArgs {
                use_transaction: false,
            },
        )
        .await?;

    let tbl = ctx.tbl_of::<Composite>().await?;
    assert_eq!(tbl.len(), 3);
    assert_eq!(tbl.get(&(1, 2)).unwrap().name, "A1B2");

    let one = LoadOne::<Composite>::load_one(ctx.provider(), &(1, 2)).await?;
    assert_eq!(one.unwrap().name, "A1B2");

    let one = LoadOne::<Composite>::load_one(ctx.provider(), &(2, 1)).await?;
    assert_eq!(one.unwrap().name, "A2B1");

    assert!(
        LoadOne::<Composite>::load_one(ctx.provider(), &(2, 2))
            .await?
            .is_none()
    );

    let trx = ctx.provider().transaction();
    Delete::<Composite>::delete(&trx, &(1, 2)).await?;
    trx.commit().await?;
    drop(trx);

    // Only the row matching both keys is deleted.
    assert!(
        LoadOne::<Composite>::load_one(ctx.provider(), &(1, 2))
            .await?
            .is_none()
    );

    let one = LoadOne::<Composite>::load_one(ctx.provider(), &(1, 1)).await?;
    assert_eq!(one.unwrap().name, "A1B1");

    let one = LoadOne::<Composite>::load_one(ctx.provider(), &(2, 1)).await?;
    assert_eq!(one.unwrap().name, "A2B1");

    Ok(())
}

#[derive(Clone, Ctx, Debug, MssqlDelete, MssqlLoad, PartialEq)]
#[storm(
    table = "##CompositeTbl",
    keys = "A,B",
    collection = "hash_table",
    no_test = true
)]
struct Composite {
    name: String,
}

impl Entity for Composite {
    type Key = (i32, i32);
}
