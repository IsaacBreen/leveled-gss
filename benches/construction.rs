mod support;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::time::Duration;
use support::{
    Bits, Explicit, WeightPartitioned, binary_stacks, homogeneous_stacks, linear_stack,
    structurally_build_binary_explicit, structurally_build_binary_gss,
    structurally_build_two_weight_explicit, structurally_build_two_weight_gss, weighted_stacks,
};
use weighted_gss::WeightedGss;

fn construction(c: &mut Criterion) {
    let mut single = c.benchmark_group("construction/from_owned_single_stack");
    single.measurement_time(Duration::from_secs(3));
    for depth in [8_usize, 32, 256, 4096] {
        let stack = linear_stack(depth);
        single.throughput(Throughput::Elements(depth as u64));
        single.bench_function(BenchmarkId::new("weighted_gss", depth), |b| {
            b.iter_batched(
                || stack.clone(),
                |stack| WeightedGss::from_stack(black_box(stack), Bits(1)),
                BatchSize::SmallInput,
            );
        });
        single.bench_function(BenchmarkId::new("explicit_map", depth), |b| {
            b.iter_batched(
                || stack.clone(),
                |stack| Explicit::from_entries([(black_box(stack), Bits(1))]),
                BatchSize::SmallInput,
            );
        });
    }
    single.finish();

    let mut import = c.benchmark_group("construction/from_explicit_entries");
    import.measurement_time(Duration::from_secs(3));
    for (label, stacks) in [
        ("shared_bottom_128x32", homogeneous_stacks(128, 32)),
        ("enumerated_binary_1024x10", binary_stacks(10)),
    ] {
        let homogeneous: Vec<_> = stacks
            .iter()
            .cloned()
            .map(|stack| (stack, Bits(1)))
            .collect();
        let weighted: Vec<_> = stacks
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, stack)| (stack, Bits(1_u64 << (index % 32))))
            .collect();
        import.throughput(Throughput::Elements(stacks.len() as u64));
        import.bench_function(BenchmarkId::new("weighted_gss/homogeneous", label), |b| {
            b.iter_batched(
                || stacks.clone(),
                |stacks| WeightedGss::from_stacks_with_weight(black_box(stacks), Bits(1)),
                BatchSize::LargeInput,
            );
        });
        import.bench_function(BenchmarkId::new("weighted_gss/weighted", label), |b| {
            b.iter_batched(
                || weighted.clone(),
                |entries| WeightedGss::from_stacks(black_box(entries)),
                BatchSize::LargeInput,
            );
        });
        import.bench_function(BenchmarkId::new("explicit_map/homogeneous", label), |b| {
            b.iter_batched(
                || homogeneous.clone(),
                |entries| Explicit::from_entries(black_box(entries)),
                BatchSize::LargeInput,
            );
        });
        import.bench_function(BenchmarkId::new("explicit_map/weighted", label), |b| {
            b.iter_batched(
                || weighted.clone(),
                |entries| Explicit::from_entries(black_box(entries)),
                BatchSize::LargeInput,
            );
        });
        import.bench_function(BenchmarkId::new("weight_partitioned", label), |b| {
            b.iter_batched(
                || weighted.clone(),
                |entries| WeightPartitioned::from_entries(black_box(entries)),
                BatchSize::LargeInput,
            );
        });
    }
    import.finish();

    let mut structural = c.benchmark_group("construction/structural_binary_growth");
    structural.measurement_time(Duration::from_secs(3));
    structural.sample_size(30);
    for levels in [4_usize, 8, 12, 16] {
        structural.bench_function(BenchmarkId::new("weighted_gss/homogeneous", levels), |b| {
            b.iter(|| black_box(structurally_build_binary_gss(black_box(levels))));
        });
        structural.bench_function(BenchmarkId::new("explicit_map/homogeneous", levels), |b| {
            b.iter(|| black_box(structurally_build_binary_explicit(black_box(levels))));
        });
        structural.bench_function(
            BenchmarkId::new("weighted_gss/two_stable_weights", levels),
            |b| {
                b.iter(|| black_box(structurally_build_two_weight_gss(black_box(levels))));
            },
        );
        structural.bench_function(
            BenchmarkId::new("explicit_map/two_stable_weights", levels),
            |b| {
                b.iter(|| black_box(structurally_build_two_weight_explicit(black_box(levels))));
            },
        );
    }
    structural.finish();

    let mut weights = c.benchmark_group("construction/from_explicit_entries/distinct_weights");
    weights.measurement_time(Duration::from_secs(3));
    for distinct in [1_usize, 2, 8, 32] {
        let entries = weighted_stacks(256, 32, distinct);
        weights.bench_function(BenchmarkId::new("weighted_gss", distinct), |b| {
            b.iter_batched(
                || entries.clone(),
                |entries| WeightedGss::from_stacks(black_box(entries)),
                BatchSize::LargeInput,
            );
        });
        weights.bench_function(BenchmarkId::new("weight_partitioned", distinct), |b| {
            b.iter_batched(
                || entries.clone(),
                |entries| WeightPartitioned::from_entries(black_box(entries)),
                BatchSize::LargeInput,
            );
        });
    }
    weights.finish();

    let mut deep = c.benchmark_group("construction/from_explicit_entries/deep_common_top");
    deep.measurement_time(Duration::from_secs(3));
    for depth in [256_usize, 4096, 20_000] {
        let make_stack = |bottom: u16| {
            let mut stack = Vec::with_capacity(depth);
            stack.push(bottom);
            stack.extend((1..depth).map(|value| value as u16));
            stack
        };
        let entries = vec![(make_stack(0), Bits(1)), (make_stack(1), Bits(2))];
        deep.throughput(Throughput::Elements((depth * entries.len()) as u64));
        deep.bench_function(BenchmarkId::new("weighted_gss", depth), |b| {
            b.iter_batched(
                || entries.clone(),
                |entries| WeightedGss::from_stacks(black_box(entries)),
                BatchSize::LargeInput,
            );
        });
        deep.bench_function(BenchmarkId::new("explicit_map", depth), |b| {
            b.iter_batched(
                || entries.clone(),
                |entries| Explicit::from_entries(black_box(entries)),
                BatchSize::LargeInput,
            );
        });
        deep.bench_function(BenchmarkId::new("weight_partitioned", depth), |b| {
            b.iter_batched(
                || entries.clone(),
                |entries| WeightPartitioned::from_entries(black_box(entries)),
                BatchSize::LargeInput,
            );
        });
    }
    deep.finish();
}

criterion_group!(benches, construction);
criterion_main!(benches);
