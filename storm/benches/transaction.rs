use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use storm::{
    Entity, NoopDelete, NoopLoad, NoopSave, async_cell_lock::with_deadlock_check, prelude::*,
};
use uuid::Uuid;

const ROWS: u32 = 10_000;
const OPS: u32 = 1_000;

#[derive(Ctx, NoopDelete, NoopLoad, NoopSave, PartialEq)]
struct TrxEntity {
    value: u64,
}

impl Entity for TrxEntity {
    type Key = u32;
}

fn bench_transaction(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");

    let lock = QueueRwLock::new(Ctx::default(), "bench");

    rt.block_on(with_deadlock_check(
        async {
            let ctx = lock.queue().await.expect("queue");
            let mut trx = ctx.transaction(Uuid::nil());

            trx.insert_all((0..ROWS).map(|k| (k, TrxEntity { value: k as u64 })))
                .await
                .expect("insert_all");

            let logs = trx.commit().await.expect("commit");
            logs.apply_log(ctx).await.expect("apply_log");
        },
        "setup",
    ));

    let mut group = c.benchmark_group("transaction");

    group.bench_function("insert_changed", |b| {
        b.iter(|| {
            rt.block_on(with_deadlock_check(
                async {
                    let ctx = lock.queue().await.expect("queue");
                    let mut trx = ctx.transaction(Uuid::nil());

                    for k in 0..OPS {
                        trx.insert(
                            k,
                            TrxEntity {
                                value: k as u64 + 1,
                            },
                        )
                        .await
                        .expect("insert");
                    }

                    black_box(trx.tbl_touched::<TrxEntity>())
                },
                "bench",
            ))
        });
    });

    group.bench_function("insert_unchanged", |b| {
        b.iter(|| {
            rt.block_on(with_deadlock_check(
                async {
                    let ctx = lock.queue().await.expect("queue");
                    let mut trx = ctx.transaction(Uuid::nil());

                    for k in 0..OPS {
                        trx.insert(k, TrxEntity { value: k as u64 })
                            .await
                            .expect("insert");
                    }

                    black_box(trx.tbl_touched::<TrxEntity>())
                },
                "bench",
            ))
        });
    });

    group.bench_function("remove", |b| {
        b.iter(|| {
            rt.block_on(with_deadlock_check(
                async {
                    let ctx = lock.queue().await.expect("queue");
                    let mut trx = ctx.transaction(Uuid::nil());

                    for k in 0..OPS {
                        trx.remove::<TrxEntity>(k).await.expect("remove");
                    }

                    black_box(trx.tbl_touched::<TrxEntity>())
                },
                "bench",
            ))
        });
    });

    group.bench_function("get_entity", |b| {
        b.iter(|| {
            rt.block_on(with_deadlock_check(
                async {
                    let ctx = lock.queue().await.expect("queue");
                    let mut trx = ctx.transaction(Uuid::nil());
                    let mut sum = 0;

                    for k in 0..OPS {
                        sum += trx
                            .get_entity::<TrxEntity>(&k)
                            .await
                            .expect("get_entity")
                            .map_or(0, |e| e.value);
                    }

                    black_box(sum)
                },
                "bench",
            ))
        });
    });

    group.bench_function("tbl_of_get", |b| {
        b.iter(|| {
            rt.block_on(with_deadlock_check(
                async {
                    let ctx = lock.queue().await.expect("queue");
                    let mut trx = ctx.transaction(Uuid::nil());
                    let mut sum = 0;

                    for k in 0..OPS {
                        let tbl = trx.tbl_of::<TrxEntity>().await.expect("tbl_of");
                        sum += tbl.get(&k).map_or(0, |e| e.value);
                    }

                    black_box(sum)
                },
                "bench",
            ))
        });
    });

    group.finish();
}

criterion_group!(benches, bench_transaction);
criterion_main!(benches);
