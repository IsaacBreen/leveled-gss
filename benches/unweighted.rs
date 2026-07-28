mod support;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use std::time::Duration;
use support::{ExplicitSet, binary_stacks, homogeneous_stacks};
use weighted_gss::Gss;

fn unweighted(c: &mut Criterion) {
    let mut construction = c.benchmark_group("unweighted/construction");
    construction.measurement_time(Duration::from_secs(3));
    for (label, stacks) in [
        ("shared_floor_128", homogeneous_stacks(128, 32)),
        ("binary_1024", binary_stacks(10)),
    ] {
        construction.throughput(Throughput::Elements(stacks.len() as u64));
        construction.bench_function(BenchmarkId::new("gss", label), |b| {
            b.iter(|| Gss::from_stacks_with_weight(black_box(stacks.clone()), ()));
        });
        construction.bench_function(BenchmarkId::new("explicit_set", label), |b| {
            b.iter(|| ExplicitSet::from_stacks(black_box(stacks.clone())));
        });
    }
    construction.finish();

    let mut operations = c.benchmark_group("unweighted/operations");
    operations.measurement_time(Duration::from_secs(3));
    for count in [32_usize, 128, 1024] {
        let stacks = homogeneous_stacks(count, 32);
        let gss = Gss::from_stacks_with_weight(stacks.clone(), ());
        let explicit = ExplicitSet::from_stacks(stacks);
        operations.throughput(Throughput::Elements(count as u64));
        operations.bench_function(BenchmarkId::new("push/gss", count), |b| {
            b.iter(|| black_box(gss.push(65_000)));
        });
        operations.bench_function(BenchmarkId::new("push/explicit_set", count), |b| {
            b.iter(|| black_box(explicit.push(65_000)));
        });
        operations.bench_function(BenchmarkId::new("pop/gss", count), |b| {
            b.iter(|| black_box(gss.pop()));
        });
        operations.bench_function(BenchmarkId::new("pop/explicit_set", count), |b| {
            b.iter(|| black_box(explicit.popn(1)));
        });
        operations.bench_function(BenchmarkId::new("materialize/gss", count), |b| {
            b.iter(|| black_box(gss.to_stacks(count).unwrap()));
        });
        operations.bench_function(BenchmarkId::new("materialize/explicit_set", count), |b| {
            b.iter(|| black_box(explicit.snapshot()));
        });
    }
    operations.finish();

    let mut merge = c.benchmark_group("unweighted/merge");
    merge.measurement_time(Duration::from_secs(3));
    for count in [128_usize, 1024] {
        let all = homogeneous_stacks(count + count / 2, 32);
        let left_stacks = all[..count].to_vec();
        let right_stacks = all[count / 2..].to_vec();
        let left_gss = Gss::from_stacks_with_weight(left_stacks.clone(), ());
        let right_gss = Gss::from_stacks_with_weight(right_stacks.clone(), ());
        let left_explicit = ExplicitSet::from_stacks(left_stacks);
        let right_explicit = ExplicitSet::from_stacks(right_stacks);
        merge.bench_function(BenchmarkId::new("gss", count), |b| {
            b.iter(|| black_box(left_gss.merge(&right_gss)));
        });
        merge.bench_function(BenchmarkId::new("explicit_set", count), |b| {
            b.iter(|| black_box(left_explicit.merge(&right_explicit)));
        });
    }
    merge.finish();
}

criterion_group!(benches, unweighted);
criterion_main!(benches);
