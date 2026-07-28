mod support;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::time::Duration;
use support::{
    Explicit, WeightPartitioned, binary_stacks, bottom_first_stack_checksum,
    top_first_stack_checksum, weighted_gss, weighted_stacks,
};
use weighted_gss::for_each_stack_top_first;

fn materialization(c: &mut Criterion) {
    let cases = [
        ("single_256", weighted_stacks(1, 256, 1)),
        ("shared_bottom_128", weighted_stacks(128, 32, 32)),
        (
            "enumerated_binary_1024",
            binary_stacks(10)
                .into_iter()
                .enumerate()
                .map(|(index, stack)| (stack, support::Bits(1_u64 << (index % 32))))
                .collect(),
        ),
    ];

    let mut canonical = c.benchmark_group("materialization/owned_output");
    canonical.measurement_time(Duration::from_secs(3));
    for (label, entries) in &cases {
        let count = entries.len();
        let gss = weighted_gss(entries);
        let explicit = Explicit::from_entries(entries.clone());
        let partitioned = WeightPartitioned::from_entries(entries.clone());
        canonical.throughput(Throughput::Elements(count as u64));
        canonical.bench_function(BenchmarkId::new("weighted_gss", label), |b| {
            b.iter(|| black_box(gss.to_stacks(count).unwrap()));
        });
        canonical.bench_function(BenchmarkId::new("explicit_map", label), |b| {
            b.iter(|| black_box(explicit.snapshot()));
        });
        canonical.bench_function(BenchmarkId::new("weight_partitioned", label), |b| {
            b.iter(|| black_box(partitioned.materialize()));
        });
    }
    canonical.finish();

    let mut borrowed = c.benchmark_group("materialization/borrowed_visit");
    borrowed.measurement_time(Duration::from_secs(3));
    for (label, entries) in &cases {
        let count = entries.len();
        let gss = weighted_gss(entries);
        let explicit = Explicit::from_entries(entries.clone());
        borrowed.throughput(Throughput::Elements(count as u64));
        borrowed.bench_function(BenchmarkId::new("weighted_gss", label), |b| {
            b.iter(|| {
                let mut checksum = 0_u64;
                for_each_stack_top_first(&gss, count, |stack, weight| {
                    checksum = checksum.wrapping_add(top_first_stack_checksum(stack, *weight));
                })
                .unwrap();
                black_box(checksum)
            });
        });
        borrowed.bench_function(BenchmarkId::new("explicit_map", label), |b| {
            b.iter(|| {
                let mut checksum = 0_u64;
                explicit
                    .visit_bounded(count, |stack, weight| {
                        checksum =
                            checksum.wrapping_add(bottom_first_stack_checksum(stack, weight));
                    })
                    .unwrap();
                black_box(checksum)
            });
        });
    }
    borrowed.finish();

    let mut limits = c.benchmark_group("materialization/limit_rejection");
    limits.measurement_time(Duration::from_secs(3));
    for levels in [8_usize, 10, 12] {
        let entries: Vec<_> = binary_stacks(levels)
            .into_iter()
            .enumerate()
            .map(|(index, stack)| (stack, support::Bits(1_u64 << (index % 32))))
            .collect();
        let gss = weighted_gss(&entries);
        limits.bench_function(BenchmarkId::new("to_stacks/limit_1", levels), |b| {
            b.iter(|| black_box(gss.to_stacks(1).unwrap_err()));
        });
        limits.bench_function(BenchmarkId::new("visitor/limit_1", levels), |b| {
            b.iter(|| black_box(for_each_stack_top_first(&gss, 1, |_, _| {}).unwrap_err()));
        });
    }
    limits.finish();
}

criterion_group!(benches, materialization);
criterion_main!(benches);
