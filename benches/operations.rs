mod support;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::time::Duration;
use support::{
    Bits, Explicit, WeightPartitioned, overlapping_entries, structurally_build_binary_explicit,
    structurally_build_binary_gss, weighted_gss, weighted_stacks,
};

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
        let (half_left, half_right) = overlapping_entries(count, 32);
        let disjoint = weighted_stacks(count * 2, 32, 32);
        let disjoint_left = disjoint[..count].to_vec();
        let disjoint_right = disjoint[count..].to_vec();
        let complete_left = weighted_stacks(count, 32, 32);
        let complete_right = complete_left
            .iter()
            .cloned()
            .map(|(stack, Bits(weight))| (stack, Bits(weight.rotate_left(7))))
            .collect();

        for (shape, left_entries, right_entries) in [
            ("disjoint", disjoint_left, disjoint_right),
            ("half_overlap", half_left, half_right),
            ("complete_overlap", complete_left, complete_right),
        ] {
            let left_gss = weighted_gss(&left_entries);
            let right_gss = weighted_gss(&right_entries);
            let left_explicit = Explicit::from_entries(left_entries.clone());
            let right_explicit = Explicit::from_entries(right_entries.clone());
            let left_partitioned = WeightPartitioned::from_entries(left_entries);
            let right_partitioned = WeightPartitioned::from_entries(right_entries);
            let case = format!("{shape}_{count}");
            merge.throughput(Throughput::Elements((count * 2) as u64));
            merge.bench_function(BenchmarkId::new("weighted_gss", &case), |b| {
                b.iter(|| black_box(left_gss.merge(&right_gss)));
            });
            merge.bench_function(BenchmarkId::new("explicit_map", &case), |b| {
                b.iter(|| black_box(left_explicit.merge(&right_explicit)));
            });
            merge.bench_function(BenchmarkId::new("weight_partitioned", &case), |b| {
                b.iter(|| black_box(left_partitioned.merge(&right_partitioned)));
            });
        }
    }
    merge.finish();

    let mut deep_merge = c.benchmark_group("operations/merge_deep_common_top");
    deep_merge.measurement_time(Duration::from_secs(3));
    for depth in [256_usize, 4096, 20_000] {
        let make_stack = |bottom: u16| {
            let mut stack = Vec::with_capacity(depth);
            stack.push(bottom);
            stack.extend((1..depth).map(|value| value as u16));
            stack
        };
        let left_entries = vec![(make_stack(0), Bits(1)), (make_stack(1), Bits(2))];
        let right_entries = vec![(make_stack(2), Bits(4)), (make_stack(3), Bits(8))];
        let left_gss = weighted_gss(&left_entries);
        let right_gss = weighted_gss(&right_entries);
        let left_explicit = Explicit::from_entries(left_entries.clone());
        let right_explicit = Explicit::from_entries(right_entries.clone());
        let left_partitioned = WeightPartitioned::from_entries(left_entries);
        let right_partitioned = WeightPartitioned::from_entries(right_entries);
        deep_merge.throughput(Throughput::Elements((depth * 4) as u64));
        deep_merge.bench_function(BenchmarkId::new("weighted_gss", depth), |b| {
            b.iter(|| black_box(left_gss.merge(&right_gss)));
        });
        deep_merge.bench_function(BenchmarkId::new("explicit_map", depth), |b| {
            b.iter(|| black_box(left_explicit.merge(&right_explicit)));
        });
        deep_merge.bench_function(BenchmarkId::new("weight_partitioned", depth), |b| {
            b.iter(|| black_box(left_partitioned.merge(&right_partitioned)));
        });
    }
    deep_merge.finish();

    let mut collapse = c.benchmark_group("operations/stress/wide_fanout_collapse_pop");
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

    let mut binary_pop = c.benchmark_group("operations/structural_binary_pop");
    binary_pop.measurement_time(Duration::from_secs(3));
    binary_pop.sample_size(30);
    for levels in [8_usize, 12, 16] {
        let gss = structurally_build_binary_gss(levels);
        let explicit = structurally_build_binary_explicit(levels);
        binary_pop.bench_function(BenchmarkId::new("weighted_gss", levels), |b| {
            b.iter(|| black_box(gss.pop()));
        });
        binary_pop.bench_function(BenchmarkId::new("explicit_map", levels), |b| {
            b.iter(|| black_box(explicit.popn(1)));
        });
    }
    binary_pop.finish();

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
