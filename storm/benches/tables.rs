use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use storm::{Ctx, Entity, HashTable, NoopLoad, VecTable};

#[derive(Ctx, NoopLoad)]
struct BenchEntity {
    value: u64,
}

impl Entity for BenchEntity {
    type Key = u32;
}

fn hash_table(size: u32) -> HashTable<BenchEntity> {
    let mut table = HashTable::new();
    table.extend((0..size).map(|key| (key, BenchEntity { value: key as u64 })));
    table
}

fn vec_table(size: u32) -> VecTable<BenchEntity> {
    let mut table = VecTable::new();
    table.extend((0..size).map(|key| (key, BenchEntity { value: key as u64 })));
    table
}

fn bench_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("table_lookup");

    for size in [10_000, 100_000, 1_000_000] {
        let hash = hash_table(size);
        group.bench_with_input(BenchmarkId::new("hash_table", size), &size, |b, &size| {
            b.iter(|| {
                let key = black_box(size / 2);
                black_box(hash.get(&key).map(|entity| entity.value))
            });
        });

        let vec = vec_table(size);
        group.bench_with_input(BenchmarkId::new("vec_table", size), &size, |b, &size| {
            b.iter(|| {
                let key = black_box(size / 2);
                black_box(vec.get(&key).map(|entity| entity.value))
            });
        });
    }

    group.finish();
}

fn bench_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("table_iteration");

    for size in [10_000, 100_000, 1_000_000] {
        let hash = hash_table(size);
        group.bench_with_input(BenchmarkId::new("hash_table", size), &size, |b, _| {
            b.iter(|| black_box(hash.values().map(|entity| entity.value).sum::<u64>()));
        });

        let vec = vec_table(size);
        group.bench_with_input(BenchmarkId::new("vec_table", size), &size, |b, _| {
            b.iter(|| black_box(vec.values().map(|entity| entity.value).sum::<u64>()));
        });
    }

    group.finish();
}

criterion_group!(benches, bench_lookup, bench_iteration);
criterion_main!(benches);
