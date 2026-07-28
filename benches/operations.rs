mod support;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::time::Duration;
use support::{Explicit, WeightPartitioned, overlapping_entries, weighted_gss, weighted_stacks};

fn operations(c: &mut Criterion) {
    let mut linear = c.benchmark_group("operations/linear");
    linear.measurement_time(Duration::from_secs(3));
    for depth in [32_usize, 256, 4096] {
        let entries = weighted_stacks(1, depth, 1);
        let gss = weighted_gss(&entries);
        let explicit = Explicit::from_entries(entries.clone());
        let partitioned = WeightPartitioned::from_entries(entries);
        linear.bench_with_input(
            BenchmarkId::new("push/weighted_gss", depth),
            &gss,
            |b, value| {
                b.iter(|| black_box(value.push(65_000)));
            },
        );
        linear.bench_with_input(
            BenchmarkId::new("push/explicit_map", depth),
            &explicit,
            |b, value| {
                b.iter(|| black_box(value.push(65_000)));
            },
        );
        linear.bench_with_input(
            BenchmarkId::new("pop/weighted_gss", depth),
            &gss,
            |b, value| {
                b.iter(|| black_box(value.pop()));
            },
        );
        linear.bench_with_input(
            BenchmarkId::new("pop/explicit_map", depth),
            &explicit,
            |b, value| {
                b.iter(|| black_box(value.popn(1)));
            },
        );
        linear.bench_with_input(
            BenchmarkId::new("push/weight_partitioned", depth),
            &partitioned,
            |b, value| b.iter(|| black_box(value.push(65_000))),
        );
    }
    linear.finish();

    let mut merge = c.benchmark_group("operations/merge");
    merge.measurement_time(Duration::from_secs(3));
    for count in [32_usize, 128, 512] {
        let (left_entries, right_entries) = overlapping_entries(count, 32);
        let left_gss = weighted_gss(&left_entries);
        let right_gss = weighted_gss(&right_entries);
        let left_explicit = Explicit::from_entries(left_entries.clone());
        let right_explicit = Explicit::from_entries(right_entries.clone());
        let left_partitioned = WeightPartitioned::from_entries(left_entries);
        let right_partitioned = WeightPartitioned::from_entries(right_entries);
        merge.throughput(Throughput::Elements((count * 2) as u64));
        merge.bench_function(BenchmarkId::new("weighted_gss/half_overlap", count), |b| {
            b.iter(|| black_box(left_gss.merge(&right_gss)));
        });
        merge.bench_function(BenchmarkId::new("explicit_map/half_overlap", count), |b| {
            b.iter(|| black_box(left_explicit.merge(&right_explicit)));
        });
        merge.bench_function(
            BenchmarkId::new("weight_partitioned/half_overlap", count),
            |b| {
                b.iter(|| black_box(left_partitioned.merge(&right_partitioned)));
            },
        );
    }
    merge.finish();

    let mut collapse = c.benchmark_group("operations/join_heavy_pop");
    collapse.measurement_time(Duration::from_secs(3));
    for count in [16_usize, 128, 1024] {
        let entries = weighted_stacks(count, 32, 32);
        let gss = weighted_gss(&entries);
        let explicit = Explicit::from_entries(entries.clone());
        let partitioned = WeightPartitioned::from_entries(entries);
        collapse.throughput(Throughput::Elements(count as u64));
        collapse.bench_function(BenchmarkId::new("weighted_gss", count), |b| {
            b.iter(|| black_box(gss.pop()));
        });
        collapse.bench_function(BenchmarkId::new("explicit_map", count), |b| {
            b.iter(|| black_box(explicit.popn(1)));
        });
        collapse.bench_function(BenchmarkId::new("weight_partitioned", count), |b| {
            b.iter(|| black_box(partitioned.popn(1)));
        });
    }
    collapse.finish();

    let mut select = c.benchmark_group("operations/retain_top");
    select.measurement_time(Duration::from_secs(3));
    for count in [32_usize, 128, 1024] {
        let entries = weighted_stacks(count, 32, 32);
        let selected_top = entries[count / 2].0.last().copied().unwrap();
        let gss = weighted_gss(&entries);
        let explicit = Explicit::from_entries(entries.clone());
        let partitioned = WeightPartitioned::from_entries(entries);
        select.bench_function(BenchmarkId::new("weighted_gss", count), |b| {
            b.iter(|| black_box(gss.retain_top(&selected_top)));
        });
        select.bench_function(BenchmarkId::new("explicit_map", count), |b| {
            b.iter(|| black_box(explicit.retain_top(selected_top)));
        });
        select.bench_function(BenchmarkId::new("weight_partitioned", count), |b| {
            b.iter(|| black_box(partitioned.retain_top(selected_top)));
        });
    }
    select.finish();

    let mut persistent = c.benchmark_group("operations/persistent_fork");
    persistent.measurement_time(Duration::from_secs(3));
    let entries = weighted_stacks(512, 64, 32);
    let gss = weighted_gss(&entries);
    let explicit = Explicit::from_entries(entries);
    persistent.bench_function("weighted_gss", |b| {
        b.iter(|| {
            let left = gss.push(60_000);
            let right = gss.push(60_001);
            black_box((left, right))
        });
    });
    persistent.bench_function("explicit_map", |b| {
        b.iter(|| {
            let left = explicit.push(60_000);
            let right = explicit.push(60_001);
            black_box((left, right))
        });
    });
    persistent.finish();
}

criterion_group!(benches, operations);
criterion_main!(benches);
