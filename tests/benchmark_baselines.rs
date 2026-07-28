#[path = "../benches/support/mod.rs"]
mod benchmark_support;

use benchmark_support::{
    Bits, Explicit, ExplicitSet, WeightPartitioned, homogeneous_stacks, weighted_gss,
    weighted_stacks,
};
use std::collections::{BTreeMap, BTreeSet};
use weighted_gss::Gss;

fn canonical(entries: impl IntoIterator<Item = (Vec<u16>, Bits)>) -> BTreeMap<Vec<u16>, Bits> {
    entries.into_iter().collect()
}

#[test]
fn weighted_benchmark_baselines_have_the_same_extensional_result() {
    let left_entries = weighted_stacks(64, 16, 16);
    let mut right_entries = weighted_stacks(64, 16, 16);
    for (index, (stack, weight)) in right_entries.iter_mut().enumerate() {
        stack[0] = stack[0].wrapping_add((index % 3) as u16);
        weight.0 = weight.0.rotate_left(5);
    }

    let gss = weighted_gss(&left_entries)
        .merge(&weighted_gss(&right_entries))
        .push(40_000)
        .pop()
        .retain_top(&left_entries[17].0.last().copied().unwrap());
    let explicit = Explicit::from_entries(left_entries.clone())
        .merge(&Explicit::from_entries(right_entries.clone()))
        .push(40_000)
        .popn(1)
        .retain_top(left_entries[17].0.last().copied().unwrap());
    let partitioned = WeightPartitioned::from_entries(left_entries)
        .merge(&WeightPartitioned::from_entries(right_entries))
        .push(40_000)
        .popn(1)
        .retain_top(gss.top().unwrap());

    let expected = canonical(gss.to_stacks(1_000).unwrap());
    assert_eq!(canonical(explicit.snapshot()), expected);
    assert_eq!(canonical(partitioned.materialize()), expected);
}

#[test]
fn unweighted_benchmark_baseline_matches_gss() {
    let left = homogeneous_stacks(128, 24);
    let mut right = homogeneous_stacks(128, 24);
    for stack in &mut right {
        stack[0] = stack[0].wrapping_add(1);
    }

    let gss = Gss::from_stacks_with_weight(left.clone(), ())
        .merge(&Gss::from_stacks_with_weight(right.clone(), ()))
        .push(50_000)
        .pop();
    let explicit = ExplicitSet::from_stacks(left)
        .merge(&ExplicitSet::from_stacks(right))
        .push(50_000)
        .popn(1);

    let actual: BTreeSet<_> = gss
        .to_stacks(1_000)
        .unwrap()
        .into_iter()
        .map(|(stack, ())| stack)
        .collect();
    assert_eq!(
        explicit.snapshot().into_iter().collect::<BTreeSet<_>>(),
        actual
    );
}
