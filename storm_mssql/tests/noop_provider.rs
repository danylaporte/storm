#![allow(clippy::unwrap_used)]

use storm::{
    MssqlDelete, MssqlLoad, MssqlSave, Result,
    prelude::*,
    provider::{LoadOne, NoopFactory},
};
use storm_mssql::MssqlProvider;
use uuid::Uuid;

fn create_ctx() -> QueueRwLock<Ctx> {
    let mut provider = ProviderContainer::new();
    provider.register("", NoopFactory);
    QueueRwLock::new(provider.into(), "ctx")
}

#[async_cell_lock::tokio_test]
async fn noop_crud_keeps_data_in_tables() -> Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.read().await?;

    assert!(ctx.tbl_of::<KeyIdentity>().await?.get(&1).is_none());
    assert!(
        LoadOne::<KeyIdentity>::load_one(ctx.provider(), &1)
            .await?
            .is_none()
    );

    let ctx = ctx.queue().await?;
    let mut trx = ctx.transaction(Uuid::nil());

    let mut tbl = trx.tbl_of::<KeyIdentity>().await?;
    let (k1, _) = tbl.insert_mut(0, KeyIdentity { name: "A".into() }).await?;
    let (k2, _) = tbl.insert_mut(0, KeyIdentity { name: "B".into() }).await?;
    assert_ne!(k1, 0);
    assert_ne!(k1, k2);

    let (k1_again, _) = tbl
        .insert_mut(k1, KeyIdentity { name: "A2".into() })
        .await?;
    assert_eq!(k1_again, k1);
    tbl.remove(k2).await?;

    let mut fields = trx.tbl_of::<FieldIdentity>().await?;
    fields.insert_mut(1, FieldIdentity { number: 0 }).await?;
    let number = fields.get(&1).unwrap().number;
    assert_ne!(number, 0);
    fields.insert_mut(1, FieldIdentity { number }).await?;
    assert_eq!(fields.get(&1).unwrap().number, number);

    let log = trx.commit().await?;
    let mut ctx = ctx.write().await?;
    ctx.apply_log(log);
    ctx.gc();

    let ctx = ctx.read().await?;
    let tbl = ctx.tbl_of::<KeyIdentity>().await?;
    assert_eq!(tbl.get(&k1).unwrap().name, "A2");
    assert!(tbl.get(&k2).is_none());
    assert_eq!(
        ctx.tbl_of::<FieldIdentity>().await?.get(&1).unwrap().number,
        number
    );

    Ok(())
}

#[async_cell_lock::tokio_test]
async fn noop_foreign_keys() -> Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.queue().await?;
    let mut trx = ctx.transaction(Uuid::nil());

    let mut parents = trx.tbl_of::<KeyIdentity>().await?;
    let (p1, _) = parents
        .insert_mut(0, KeyIdentity { name: "A".into() })
        .await?;
    let (p2, _) = parents
        .insert_mut(0, KeyIdentity { name: "B".into() })
        .await?;

    let mut children = trx.tbl_of::<FkSetNullChild>().await?;
    children
        .insert_mut(
            1,
            FkSetNullChild {
                parent_id: Some(p1),
                number: 0,
            },
        )
        .await?;
    children
        .insert_mut(
            2,
            FkSetNullChild {
                parent_id: Some(p2),
                number: 0,
            },
        )
        .await?;

    trx.insert(1, FkCascadeChild { parent_id: p1 }).await?;
    trx.insert(2, FkCascadeChild { parent_id: p2 }).await?;

    assert!(trx.remove::<KeyIdentity>(p1).await?);

    let children = trx.tbl_of::<FkSetNullChild>().await?;
    assert_eq!(children.get(&1).unwrap().parent_id, None);
    assert_eq!(children.get(&2).unwrap().parent_id, Some(p2));

    let cascade = trx.tbl_of::<FkCascadeChild>().await?;
    assert!(cascade.get(&1).is_none());
    assert!(cascade.get(&2).is_some());

    assert!(
        trx.insert(3, FkCascadeChild { parent_id: p1 })
            .await
            .is_err()
    );

    Ok(())
}

#[async_cell_lock::tokio_test]
async fn noop_unique() -> Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.queue().await?;
    let mut trx = ctx.transaction(Uuid::nil());

    let mut items = trx.tbl_of::<UniqueItem>().await?;
    items
        .insert_mut(
            1,
            UniqueItem {
                code: "Café".into(),
                group: 1,
                number: 0,
            },
        )
        .await?;
    items
        .insert_mut(
            2,
            UniqueItem {
                code: "other".into(),
                group: 1,
                number: 0,
            },
        )
        .await?;

    assert!(matches!(
        items
            .insert_mut(
                3,
                UniqueItem {
                    code: "CAFE".into(),
                    group: 2,
                    number: 0
                }
            )
            .await,
        Err(storm::Error::UniqueViolation { index: "code", .. })
    ));

    Ok(())
}

#[async_cell_lock::tokio_test]
async fn noop_max_length() -> Result<()> {
    let ctx = create_ctx();
    let ctx = ctx.queue().await?;

    // measured in UTF-16 code units, like nvarchar: an emoji counts for 2.
    {
        let mut trx = ctx.transaction(Uuid::nil());
        trx.insert(
            1,
            Sized5 {
                code: "a😀😀".into(),
                note: Some("abcde".into()),
            },
        )
        .await?;
        assert_eq!(Sized5::CODE_MAX_LENGTH, 5);
    }

    {
        let mut trx = ctx.transaction(Uuid::nil());
        let err = trx
            .insert(
                2,
                Sized5 {
                    code: "ab😀😀".into(),
                    note: None,
                },
            )
            .await
            .unwrap_err();

        assert_eq!(err.to_string(), "code field too long, len: 6, max 5");
    }

    // every field too long is reported.
    let mut trx = ctx.transaction(Uuid::nil());
    let err = trx
        .insert(
            2,
            Sized5 {
                code: "abcdef".into(),
                note: Some("é".repeat(6).into()),
            },
        )
        .await
        .unwrap_err();

    let storm::Error::Multiple(errors) = err else {
        panic!("{err:?}");
    };

    let errors = errors.iter().map(ToString::to_string).collect::<Vec<_>>();

    assert_eq!(
        errors,
        [
            "code field too long, len: 6, max 5",
            "note field too long, len: 6, max 5"
        ]
    );

    Ok(())
}

#[tokio::test]
#[should_panic(expected = "noop mode")]
async fn noop_raw_provider_access_panics() {
    let mut provider = ProviderContainer::new();
    provider.register("", NoopFactory);
    let _ = provider.provide::<MssqlProvider>("").await;
}

#[derive(Clone, Ctx, Debug, MssqlDelete, MssqlLoad, MssqlSave, PartialEq)]
#[storm(
    table = "KeyIdentity",
    keys = "Id",
    collection = "hash_table",
    identity = "id",
    no_test = true
)]
struct KeyIdentity {
    name: String,
}

impl Entity for KeyIdentity {
    type Key = i32;
}

#[derive(Clone, Ctx, Debug, MssqlDelete, MssqlLoad, MssqlSave, PartialEq)]
#[storm(
    table = "FieldIdentity",
    keys = "Id",
    collection = "hash_table",
    identity = "number",
    no_test = true
)]
struct FieldIdentity {
    number: i64,
}

impl Entity for FieldIdentity {
    type Key = i32;
}

#[derive(Clone, Ctx, Debug, MssqlDelete, MssqlLoad, MssqlSave, PartialEq)]
#[storm(
    table = "FkSetNullChild",
    keys = "Id",
    identity = "number",
    no_test = true
)]
struct FkSetNullChild {
    #[storm(foreign_key(entity = KeyIdentity, on_delete = "set_null", hash))]
    parent_id: Option<i32>,
    number: i64,
}

impl Entity for FkSetNullChild {
    type Key = u8;
}

#[derive(Clone, Ctx, Debug, MssqlDelete, MssqlLoad, MssqlSave, PartialEq)]
#[storm(table = "FkCascadeChild", keys = "Id", no_test = true)]
struct FkCascadeChild {
    #[storm(foreign_key(entity = KeyIdentity, on_delete = "cascade", hash))]
    parent_id: i32,
}

impl Entity for FkCascadeChild {
    type Key = u8;
}

#[derive(Clone, Ctx, Debug, MssqlDelete, MssqlLoad, MssqlSave, PartialEq)]
#[storm(
    table = "UniqueItem",
    keys = "Id",
    identity = "number",
    unique(fields = "group, number"),
    no_test = true
)]
struct UniqueItem {
    #[storm(unique(collation = "ai_ci"))]
    code: String,
    group: i32,
    number: i64,
}

impl Entity for UniqueItem {
    type Key = u8;
}

#[derive(Clone, Ctx, Debug, MssqlDelete, MssqlLoad, MssqlSave, PartialEq)]
#[storm(table = "Sized5", keys = "Id", no_test = true)]
struct Sized5 {
    #[storm(max_length = 5)]
    code: String,
    #[storm(max_length = 5)]
    note: Option<Box<str>>,
}

impl Entity for Sized5 {
    type Key = u8;
}
