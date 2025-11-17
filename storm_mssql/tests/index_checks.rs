use storm::{
    flat_set_index, hash_flat_set_index, indexing, one_index, single_set, tree_index, Ctx, Entity,
    Error, Gc, MssqlDelete, MssqlLoad, MssqlSave, ProviderContainer, QueueRwLock, Result,
    Transaction,
};
use storm_mssql::{Execute, ExecuteArgs, FromSql, MssqlFactory, MssqlProvider, ToSql, ToSqlNull};
use tiberius::Config;
use uuid::Uuid;

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

#[tokio::test]
async fn flat_sets() -> Result<()> {
    async_cell_lock::with_deadlock_check(
        async move {
            let global = create_ctx();
            {
                let ctx = global.read().await?;

                eval_sql(
                    &ctx,
                    "CREATE TABLE ##MyEntities (Id INT PRIMARY KEY NOT NULL, Fk INT NULL, Parent INT NULL);",
                )
                .await?;
            }

            {
                let ctx = global.queue().await?;
                let mut trx = ctx.transaction(Uuid::nil());

                trx.ref_as::<FlatIdx>().await?;
                trx.insert(PkId(1), MyEntity { fk: Some(FkId(2)), parent: None }).await?;

                assert!(trx.index::<FlatIdx>().await?.contains(FkId(2), PkId(1)));

                assert!(trx
                    .index::<HashIdx>()
                    .await?
                    .contains(&FkId(2), PkId(1)));

                trx.insert(PkId(2), MyEntity { fk: None, parent: Some(PkId(1)) }).await?;

                assert!(trx.index::<FlatIdx>().await?.contains_none(PkId(2)));
                assert!(trx.index::<TreeIdx>().await?.descendants(PkId(1)).contains(PkId(2)));

                assert!(trx.commit().await?.apply_log(ctx).await?);
            }

            {
                let ctx = global.read().await?;
                let idx = ctx.ref_as::<FlatIdx>().await?;

                assert!(idx.contains(FkId(2), PkId(1)));
                assert!(idx.contains_none(PkId(2)));

                assert_eq!(
                    ctx.ref_as::<ReFlatIdx>().await?.0,
                    vec![(PkId(1), FkId(2))]
                );

                assert!(ctx
                    .ref_as::<HashIdx>()
                    .await?
                    .contains(&FkId(2), PkId(1)));

                assert_eq!(ctx.ref_as::<OneIdx>().await?.get(PkId(1)), Some(&FkId(2)));
                assert_eq!(ctx.ref_as::<ReOneIdx>().await?.0, vec![(PkId(1), FkId(2))]);

                assert!(ctx.ref_as::<SingleIdx>().await?.contains(PkId(1)));
                assert_eq!(ctx.ref_as::<ReSingleIdx>().await?.0, vec![PkId(1)]);
                assert!(ctx.ref_as::<TreeIdx>().await?.descendants(PkId(1)).contains(PkId(2)));
                assert_eq!(ctx.ref_as::<ReTreeIdx>().await?.0, vec![PkId(1), PkId(2)]);

                assert_eq!(ctx
                    .ref_as::<ReHashIdx>()
                    .await?
                    .0, vec![(PkId(1), FkId(2))]);

                eval_sql(&ctx, "INSERT ##MyEntities (Id, Fk, Parent) VALUES (3, 6, 2);").await?;
            }

            {
                // force a clear of the table.
                let mut ctx = global.queue().await?.write().await?;
                ctx.clear_tbl_of::<MyEntity>();
            }

            {
                let ctx = global.queue().await?;

                assert_eq!(ctx.ref_as::<OneIdx>().await?.get(PkId(3)), Some(&FkId(6)));
                assert_eq!(ctx.ref_as::<ReOneIdx>().await?.0, vec![(PkId(1), FkId(2)), (PkId(3), FkId(6))]);

                assert!(ctx.ref_as::<SingleIdx>().await?.contains(PkId(1)));
                assert!(ctx.ref_as::<SingleIdx>().await?.contains(PkId(3)));
                assert_eq!(ctx.ref_as::<ReSingleIdx>().await?.0, vec![PkId(1), PkId(3)]);

                assert_eq!(ctx
                    .ref_as::<ReHashIdx>()
                    .await?
                    .0, vec![(PkId(1), FkId(2)), (PkId(3), FkId(6))]);

                assert_eq!(ctx.ref_as::<ReTreeIdx>().await?.0, vec![PkId(1), PkId(2), PkId(3)]);

                let mut trx = ctx.transaction(Uuid::nil());

                assert_eq!(
                    ctx.ref_as::<ReFlatIdx>().await?.0,
                    vec![(PkId(1), FkId(2)), (PkId(3), FkId(6))]
                );

                assert!(trx.index::<FlatIdx>().await?.contains(FkId(6), PkId(3)));

                assert!(trx
                    .index::<HashIdx>()
                    .await?
                    .contains(&FkId(6), PkId(3)));

                assert!(trx.index::<TreeIdx>().await?.descendants(PkId(2)).contains(PkId(3)));

                trx.remove::<MyEntity>(PkId(1)).await?;
                trx.remove::<MyEntity>(PkId(2)).await?;

                assert!(!trx.index::<FlatIdx>().await?.contains(FkId(2), PkId(1)));

                assert!(!trx
                    .index::<HashIdx>()
                    .await?
                    .contains(&FkId(2), PkId(1)));

                assert!(trx.index::<TreeIdx>().await?.descendants(PkId(1)).is_empty());
                assert!(trx.index::<TreeIdx>().await?.descendants(PkId(2)).is_empty());

                trx.commit().await?.apply_log(ctx).await?;
            }

            {
                let ctx = global.read().await?;
                let idx = ctx.ref_as::<FlatIdx>().await?;

                assert!(!idx.contains(FkId(2), PkId(1)));
                assert!(!idx.contains_none(PkId(2)));

                assert_eq!(
                    ctx.ref_as::<ReFlatIdx>().await?.0,
                    vec![(PkId(3), FkId(6))]
                );

                assert!(!ctx
                    .ref_as::<HashIdx>()
                    .await?
                    .contains(&FkId(2), PkId(1)));

                assert_eq!(ctx
                    .ref_as::<ReHashIdx>()
                    .await?
                    .0, vec![(PkId(3), FkId(6))]);

                assert_eq!(ctx.ref_as::<OneIdx>().await?.get(PkId(3)), Some(&FkId(6)));
                assert_eq!(ctx.ref_as::<ReOneIdx>().await?.0, vec![(PkId(3), FkId(6))]);
                assert!(!ctx.ref_as::<SingleIdx>().await?.contains(PkId(1)));
                assert!(ctx.ref_as::<SingleIdx>().await?.contains(PkId(3)));
                assert_eq!(ctx.ref_as::<ReSingleIdx>().await?.0, vec![PkId(3)]);

                assert!(ctx.ref_as::<TreeIdx>().await?.descendants(PkId(1)).is_empty());
                assert!(ctx.ref_as::<TreeIdx>().await?.descendants(PkId(2)).is_empty());
                assert!(ctx.ref_as::<ReTreeIdx>().await?.0.is_empty());
            }

            Ok(())
        },
        "flat_set",
    )
    .await
}

async fn eval_sql<S>(ctx: &Ctx, sql: S) -> Result<()>
where
    S: Into<String>,
{
    let provider = ctx.provider().provide::<MssqlProvider>("").await?;
    let no_transaction = ExecuteArgs {
        use_transaction: false,
    };

    provider
        .execute_with_args(sql.into(), &[], no_transaction)
        .await?;

    Ok(())
}

#[derive(Ctx, MssqlDelete, MssqlLoad, MssqlSave, PartialEq)]
#[storm(table = "##MyEntities", keys = "id")]
struct MyEntity {
    fk: Option<FkId>,
    parent: Option<PkId>,
}

impl Entity for MyEntity {
    type Key = PkId;
}

#[flat_set_index]
fn flat_idx(id: &PkId, entity: &MyEntity) -> Option<(Option<FkId>, PkId)> {
    Some((entity.fk, *id))
}

#[hash_flat_set_index]
fn hash_idx(id: &PkId, entity: &MyEntity) -> Option<(Option<FkId>, PkId)> {
    Some((entity.fk, *id))
}

#[one_index]
fn one_idx(id: &PkId, entity: &MyEntity) -> Option<FkId> {
    entity.fk
}

#[indexing]
fn re_flat_idx(pk_ids_by_fk_id: &FlatIdx) -> Vec<(PkId, FkId)> {
    let mut vec = Vec::new();

    for (k, v) in pk_ids_by_fk_id.iter() {
        for v in v {
            vec.push((v, k));
        }
    }

    vec.sort_unstable();
    vec
}

#[indexing]
fn re_hash_idx(pk_ids_by_fk_id: &HashIdx) -> Vec<(PkId, FkId)> {
    let mut vec = Vec::new();

    for (k, v) in pk_ids_by_fk_id.iter() {
        for v in v {
            vec.push((v, *k));
        }
    }

    vec.sort_unstable();
    vec
}

#[indexing]
fn re_one_idx(one_idx: &OneIdx) -> Vec<(PkId, FkId)> {
    one_idx.iter().map(|(a, b)| (a, *b)).collect()
}

#[indexing]
fn re_single_idx(single_idx: &SingleIdx) -> Vec<PkId> {
    let mut v = single_idx.iter().collect::<Vec<_>>();
    v.sort_unstable();
    v
}

#[indexing]
fn re_tree_idx(tree_idx: &TreeIdx) -> Vec<PkId> {
    let mut v = tree_idx.all_nodes().iter().collect::<Vec<_>>();
    v.sort_unstable();
    v
}

#[single_set]
fn single_idx(id: &PkId, entity: &MyEntity) -> bool {
    entity.fk.is_some()
}

#[tree_index]
fn tree_idx(entity: &MyEntity) -> Option<PkId> {
    entity.parent
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PkId(i32);

impl<'a> FromSql<'a> for PkId {
    type Column = i32;

    fn from_sql(col: Option<Self::Column>) -> Result<Self> {
        match col {
            Some(col) => Ok(PkId(col)),
            None => Err(Error::ColumnNull),
        }
    }
}

impl ToSql for PkId {
    fn to_sql(&self) -> tiberius::ColumnData<'_> {
        tiberius::ColumnData::I32(Some(self.0))
    }
}

impl ToSqlNull for PkId {
    fn to_sql_null() -> tiberius::ColumnData<'static> {
        tiberius::ColumnData::I32(None)
    }
}

impl From<PkId> for u32 {
    fn from(id: PkId) -> Self {
        id.0 as _
    }
}

impl Gc for PkId {}

impl TryFrom<u32> for PkId {
    type Error = ();

    fn try_from(value: u32) -> std::result::Result<Self, Self::Error> {
        Ok(Self(value as i32))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FkId(i32);

impl<'a> FromSql<'a> for FkId {
    type Column = i32;

    fn from_sql(col: Option<Self::Column>) -> Result<Self> {
        match col {
            Some(col) => Ok(FkId(col)),
            None => Err(Error::ColumnNull),
        }
    }
}

impl From<FkId> for u32 {
    fn from(id: FkId) -> Self {
        id.0 as _
    }
}

impl Gc for FkId {}

impl ToSql for FkId {
    fn to_sql(&self) -> tiberius::ColumnData<'_> {
        tiberius::ColumnData::I32(Some(self.0))
    }
}

impl ToSqlNull for FkId {
    fn to_sql_null() -> tiberius::ColumnData<'static> {
        tiberius::ColumnData::I32(None)
    }
}

impl TryFrom<u32> for FkId {
    type Error = ();
    
    fn try_from(value: u32) -> std::result::Result<Self, Self::Error> {
        Ok(Self(value as i32))
    }
}
