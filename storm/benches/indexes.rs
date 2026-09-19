#![allow(clippy::expect_used)]

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use storm::{
    Ctx, Entity, NoopDelete, NoopLoad, NoopSave, QueueRwLock, Result, Transaction, VecTable,
    flat_set_index, hash_flat_set_index,
    indexing::{FlatSetAdapt, HashFlatSetAdapt, OneAdapt, SingleSetAdapt},
    one_index, single_set,
};
use tokio::runtime::Runtime;
use uuid::Uuid;

const GROUPS: u32 = 100;

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct Row {
    group: u32,
    flag: bool,
}

impl Entity for Row {
    type Key = u32;
}

#[flat_set_index]
fn rows_by_group(id: &u32, row: &Row) -> Option<(Option<u32>, u32)> {
    Some((Some(row.group), *id))
}

#[hash_flat_set_index]
fn rows_by_group_hash(id: &u32, row: &Row) -> Option<(Option<u32>, u32)> {
    Some((Some(row.group), *id))
}

#[one_index]
fn group_of_row(_id: &u32, row: &Row) -> Option<u32> {
    Some(row.group)
}

#[single_set]
fn flagged_rows(_id: &u32, row: &Row) -> bool {
    row.flag
}

fn row(key: u32) -> Row {
    Row {
        group: key % GROUPS + 1,
        flag: key % 2 == 0,
    }
}

fn table(size: u32) -> VecTable<Row> {
    let mut table = VecTable::new();
    table.extend((0..size).map(|key| (key, row(key))));
    table
}

fn runtime() -> Runtime {
    Runtime::new().expect("tokio runtime")
}

async fn populated_ctx(size: u32) -> Result<QueueRwLock<Ctx>> {
    let lock = QueueRwLock::new(Ctx::default(), "bench");

    {
        let ctx = lock.queue().await?;
        let mut trx = ctx.transaction(Uuid::nil());

        trx.insert_all((0..size).map(|key| (key, row(key)))).await?;
        trx.commit().await?.apply_log(ctx).await?;
    }

    {
        // Initialize every index so transactions measure log maintenance only.
        let ctx = lock.read().await?;
        ctx.ref_as::<RowsByGroup>().await?;
        ctx.ref_as::<RowsByGroupHash>().await?;
        ctx.ref_as::<GroupOfRow>().await?;
        ctx.ref_as::<FlaggedRows>().await?;
    }

    Ok(lock)
}

fn bench_index_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_build");

    for size in [10_000, 100_000] {
        let tbl = table(size);

        group.bench_with_input(BenchmarkId::new("flat_set", size), &tbl, |b, tbl| {
            b.iter_batched(
                Ctx::default,
                |ctx| {
                    black_box(RowsByGroupAdapt::get_or_init_sync(&ctx, tbl));
                    ctx
                },
                BatchSize::LargeInput,
            );
        });

        group.bench_with_input(BenchmarkId::new("hash_flat_set", size), &tbl, |b, tbl| {
            b.iter_batched(
                Ctx::default,
                |ctx| {
                    black_box(RowsByGroupHashAdapt::get_or_init_sync(&ctx, tbl));
                    ctx
                },
                BatchSize::LargeInput,
            );
        });

        group.bench_with_input(BenchmarkId::new("one", size), &tbl, |b, tbl| {
            b.iter_batched(
                Ctx::default,
                |ctx| {
                    black_box(GroupOfRowAdapt::get_or_init_sync(&ctx, tbl));
                    ctx
                },
                BatchSize::LargeInput,
            );
        });

        group.bench_with_input(BenchmarkId::new("single_set", size), &tbl, |b, tbl| {
            b.iter_batched(
                Ctx::default,
                |ctx| {
                    black_box(FlaggedRowsAdapt::get_or_init_sync(&ctx, tbl));
                    ctx
                },
                BatchSize::LargeInput,
            );
        });
    }

    group.finish();
}

fn bench_transaction_updates(c: &mut Criterion) {
    let rt = runtime();
    let mut group = c.benchmark_group("transaction_updates");

    for size in [10_000, 100_000] {
        let lock = rt
            .block_on(async_cell_lock::with_deadlock_check(
                populated_ctx(size),
                "setup",
            ))
            .expect("populated ctx");

        for changes in [1u32, 100] {
            let id = BenchmarkId::new(format!("update_{changes}"), size);

            group.bench_with_input(id, &changes, |b, &changes| {
                b.iter(|| {
                    rt.block_on(async_cell_lock::with_deadlock_check(
                        async {
                            let ctx = lock.queue().await?;
                            let mut trx = ctx.transaction(Uuid::nil());

                            for key in 0..changes {
                                // Move the row to another group and flip its flag.
                                let mut row = row(key);
                                row.group = row.group % GROUPS + 1;
                                row.flag = !row.flag;
                                trx.insert(key, row).await?;
                            }

                            let idx = trx.index::<RowsByGroup>().await?;
                            black_box(idx.contains(2, 0));

                            let idx = trx.index::<RowsByGroupHash>().await?;
                            black_box(idx.contains(&2, 0));

                            trx.index::<FlaggedRows>().await?;

                            Ok::<(), storm::Error>(())
                        },
                        "update",
                    ))
                    .expect("transaction");
                });
            });
        }
    }

    group.finish();
}

criterion_group!(benches, bench_index_build, bench_transaction_updates);
criterion_main!(benches);
