use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use storm_mssql::{FilterSql, KeysFilter, UpsertBuilder};

fn bench_upsert_sql(c: &mut Criterion) {
    let mut group = c.benchmark_group("upsert_sql");

    for field_count in [2, 8, 32] {
        let names = (0..field_count)
            .map(|index| format!("[Value{index}]"))
            .collect::<Vec<_>>();

        group.bench_with_input(
            BenchmarkId::from_parameter(field_count),
            &names,
            |b, names| {
                b.iter(|| {
                    let id = 7_u32;
                    let value = 11_u32;
                    let mut builder = UpsertBuilder::new("[dbo].[Bench]");
                    for name in names {
                        builder.add_field_ref(name, &value);
                    }
                    builder.add_key_ref("[Id]", &id);
                    black_box(builder.sql())
                });
            },
        );
    }

    group.finish();
}

fn bench_keys_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("keys_filter");

    for key_count in [1, 16, 256] {
        let keys = (0..key_count).collect::<Vec<u32>>();
        group.bench_with_input(BenchmarkId::from_parameter(key_count), &keys, |b, keys| {
            b.iter(|| {
                let filter = KeysFilter("[Id]", keys);
                black_box(filter.filter_sql(0));
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_upsert_sql, bench_keys_filter);
criterion_main!(benches);
