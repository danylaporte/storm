//! Live SQL Server benchmarks.
//!
//! Skipped unless `STORM_MSSQL_BENCH` is set. Connects like the integration
//! tests (local default instance, `master`, integrated auth) and only touches
//! a `##BenchRows` global temp table.
#![allow(clippy::expect_used)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::sync::atomic::{AtomicI32, Ordering::Relaxed};
use storm::{
    Ctx, Entity, HashTable, MssqlDelete, MssqlLoad, MssqlSave, ProviderContainer, QueueRwLock,
    Result, Transaction, provider::LoadAll,
};
use storm_mssql::{Execute, ExecuteArgs, MssqlFactory, MssqlProvider};
use tiberius::Config;
use tokio::runtime::Runtime;
use uuid::Uuid;

const ENV: &str = "STORM_MSSQL_BENCH";

#[derive(Clone, Ctx, Debug, MssqlDelete, MssqlLoad, MssqlSave, PartialEq)]
#[storm(
    table = "##BenchRows",
    keys = "Id",
    collection = "hash_table",
    no_test = true
)]
struct BenchRow {
    name: String,
    value: i32,
    flag: bool,
}

impl Entity for BenchRow {
    type Key = i32;
}

fn enabled() -> bool {
    std::env::var_os(ENV).is_some()
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

async fn exec(provider: &ProviderContainer, sql: &'static str) -> Result<()> {
    let mssql = provider.provide::<MssqlProvider>("").await?;
    let args = ExecuteArgs {
        use_transaction: false,
    };
    mssql.execute_with_args(sql, &[], args).await?;
    Ok(())
}

async fn recreate_table(provider: &ProviderContainer, rows: i32) -> Result<()> {
    exec(
        provider,
        "IF OBJECT_ID('tempdb..##BenchRows') IS NOT NULL DROP TABLE ##BenchRows;",
    )
    .await?;

    exec(
        provider,
        "CREATE TABLE ##BenchRows (Id INT PRIMARY KEY NOT NULL, Name NVARCHAR(50) NOT NULL, Value INT NOT NULL, Flag BIT NOT NULL);",
    )
    .await?;

    let mssql = provider.provide::<MssqlProvider>("").await?;
    let args = ExecuteArgs {
        use_transaction: false,
    };

    mssql
        .execute_with_args(
            "WITH n AS (
                SELECT TOP (@p1) ROW_NUMBER() OVER (ORDER BY (SELECT NULL)) AS i
                FROM sys.all_objects a CROSS JOIN sys.all_objects b
            )
            INSERT ##BenchRows (Id, Name, Value, Flag)
            SELECT i, CONCAT('name', i), i * 2, i % 2 FROM n;",
            &[&rows],
            args,
        )
        .await?;

    Ok(())
}

fn bench_load_all(c: &mut Criterion) {
    if !enabled() {
        eprintln!("{ENV} not set; skipping live MSSQL benchmarks");
        return;
    }

    let rt = Runtime::new().expect("tokio runtime");
    let provider = provider();
    let mut group = c.benchmark_group("mssql_load_all");
    group.sample_size(20);

    for rows in [1_000, 10_000] {
        rt.block_on(recreate_table(&provider, rows))
            .expect("recreate table");

        group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, _| {
            b.iter(|| {
                let tbl: HashTable<BenchRow> = rt
                    .block_on(LoadAll::<BenchRow, (), _>::load_all(&provider, &()))
                    .expect("load_all");
                black_box(tbl.len())
            });
        });
    }

    group.finish();
}

fn bench_upsert(c: &mut Criterion) {
    if !enabled() {
        return;
    }

    let rt = Runtime::new().expect("tokio runtime");
    let lock = QueueRwLock::new(Ctx::new(provider()), "bench");

    rt.block_on(async_cell_lock::with_deadlock_check(
        async {
            let ctx = lock.read().await?;
            recreate_table(ctx.provider(), 1_000).await?;
            ctx.tbl_of::<BenchRow>().await?;
            Ok::<(), storm::Error>(())
        },
        "setup",
    ))
    .expect("setup");

    let mut group = c.benchmark_group("mssql_upsert");
    group.sample_size(20);

    // Unchanged entities are skipped by storm, so every iteration writes new values.
    let generation = AtomicI32::new(0);

    for rows in [1, 10, 100] {
        group.bench_with_input(BenchmarkId::from_parameter(rows), &rows, |b, &rows| {
            b.iter(|| {
                let generation = generation.fetch_add(1, Relaxed);

                rt.block_on(async_cell_lock::with_deadlock_check(
                    async {
                        let ctx = lock.queue().await?;
                        let mut trx = ctx.transaction(Uuid::nil());

                        for id in 1..=rows {
                            let row = BenchRow {
                                name: format!("gen{generation}"),
                                value: generation,
                                flag: id % 2 == 0,
                            };

                            trx.insert(id, row).await?;
                        }

                        trx.commit().await?.apply_log(ctx).await?;
                        Ok::<(), storm::Error>(())
                    },
                    "upsert",
                ))
                .expect("upsert");
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_load_all, bench_upsert);
criterion_main!(benches);
