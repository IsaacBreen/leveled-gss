mod support;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::time::Duration;
use support::{
    Bits, Explicit, WeightPartitioned, binary_stacks, homogeneous_stacks, linear_stack,
    weighted_stacks,
};
use weighted_gss::WeightedGss;

fn construction(c: &mut Criterion) {
    let mut single = c.benchmark_group("construction/single_stack");
    single.measurement_time(Duration::from_secs(3));
    for depth in [8_usize, 32, 256, 4096] {
        let stack = linear_stack(depth);
        single.throughput(Throughput::Elements(depth as u64));
        single.bench_with_input(
            BenchmarkId::new("weighted_gss", depth),
            &stack,
            |b, stack| {
                b.iter(|| WeightedGss::from_stack(black_box(stack.clone()), Bits(1)));
            },
        );
        single.bench_with_input(
            BenchmarkId::new("explicit_map", depth),
            &stack,
            |b, stack| {
                b.iter(|| Explicit::from_entries([(black_box(stack.clone()), Bits(1))]));
            },
        );
    }
    single.finish();

    let mut branched = c.benchmark_group("construction/branched");
    branched.measurement_time(Duration::from_secs(3));
    for (label, stacks) in [
        ("shared_floor_128x32", homogeneous_stacks(128, 32)),
        ("binary_1024x10", binary_stacks(10)),
    ] {
        let weighted: Vec<_> = stacks
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, stack)| (stack, Bits(1_u64 << (index % 32))))
            .collect();
        branched.throughput(Throughput::Elements(stacks.len() as u64));
        branched.bench_function(BenchmarkId::new("weighted_gss/homogeneous", label), |b| {
            b.iter(|| WeightedGss::from_stacks_with_weight(black_box(stacks.clone()), Bits(1)));
        });
        branched.bench_function(BenchmarkId::new("weighted_gss/weighted", label), |b| {
            b.iter(|| WeightedGss::from_stacks(black_box(weighted.clone())));
        });
        branched.bench_function(BenchmarkId::new("explicit_map", label), |b| {
            b.iter(|| Explicit::from_entries(black_box(weighted.clone())));
        });
        branched.bench_function(BenchmarkId::new("weight_partitioned", label), |b| {
            b.iter(|| WeightPartitioned::from_entries(black_box(weighted.clone())));
        });
    }
    branched.finish();

    let mut weights = c.benchmark_group("construction/distinct_weights");
    weights.measurement_time(Duration::from_secs(3));
    for distinct in [1_usize, 2, 8, 32] {
        let entries = weighted_stacks(256, 32, distinct);
        weights.bench_with_input(
            BenchmarkId::new("weighted_gss", distinct),
            &entries,
            |b, entries| b.iter(|| WeightedGss::from_stacks(black_box(entries.clone()))),
        );
        weights.bench_with_input(
            BenchmarkId::new("weight_partitioned", distinct),
            &entries,
            |b, entries| b.iter(|| WeightPartitioned::from_entries(black_box(entries.clone()))),
        );
    }
    weights.finish();

    let mut deep = c.benchmark_group("construction/deep_common_top_prefix");
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
        deep.bench_with_input(
            BenchmarkId::new("weighted_gss", depth),
            &entries,
            |b, entries| b.iter(|| WeightedGss::from_stacks(black_box(entries.clone()))),
        );
        deep.bench_with_input(
            BenchmarkId::new("explicit_map", depth),
            &entries,
            |b, entries| b.iter(|| Explicit::from_entries(black_box(entries.clone()))),
        );
        deep.bench_with_input(
            BenchmarkId::new("weight_partitioned", depth),
            &entries,
            |b, entries| b.iter(|| WeightPartitioned::from_entries(black_box(entries.clone()))),
        );
    }
    deep.finish();
}

criterion_group!(benches, construction);
criterion_main!(benches);
