#![cfg(feature = "engine")]

use std::collections::BTreeMap;
use weighted_gss::engine::{
    StackLanguageInterner, filter_map_path_weights, for_each_stack_top_first, linear_prefix,
    path_weights,
};
use weighted_gss::{Weight, WeightedGss};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bits(u8);

impl Weight for Bits {
    fn join(&self, other: &Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[test]
fn bounded_visit_coalesces_duplicate_concrete_stacks() {
    let left = WeightedGss::from_stack([0_u8, 1], Bits(1));
    let right = WeightedGss::from_stack([0_u8, 1], Bits(4));
    let gss = left.merge(&right);

    let mut visited = Vec::new();
    for_each_stack_top_first(&gss, 1, |stack, weight| {
        visited.push((stack.to_vec(), *weight));
    })
    .unwrap();

    assert_eq!(visited, vec![(vec![1, 0], Bits(5))]);
}

#[test]
fn bounded_visit_rejects_large_shared_language_without_expanding_it() {
    let mut gss = WeightedGss::from_stack(Vec::<u8>::new(), Bits(1));
    for level in 0..18_u8 {
        gss = gss.push(level * 2).merge(&gss.push(level * 2 + 1));
    }

    let error = for_each_stack_top_first(&gss, 32, |_, _| {}).unwrap_err();
    assert_eq!(error.limit, 32);
}

#[test]
fn path_weight_operations_preserve_stack_correlation() {
    let gss = WeightedGss::from_stacks([
        (vec![0_u8, 1], Bits(1)),
        (vec![0_u8, 2], Bits(2)),
        (vec![9_u8], Bits(4)),
    ]);
    assert_eq!(path_weights(&gss).count(), 3);

    let filtered = filter_map_path_weights(&gss, |weight| {
        (weight.0 != 2).then_some(Bits(weight.0 << 1))
    });
    let actual: BTreeMap<_, _> = filtered.to_stacks(8).unwrap().into_iter().collect();
    let expected = BTreeMap::from([(vec![0, 1], Bits(2)), (vec![9], Bits(8))]);
    assert_eq!(actual, expected);
}

#[test]
fn linear_prefix_mutation_preserves_hidden_floor() {
    let floor = WeightedGss::from_stacks_with_weight([vec![0_u8, 1], vec![9_u8, 1]], Bits(1));
    let gss = floor.push(7).push(8);
    let mut prefix = linear_prefix(&gss).expect("linear prefix");

    assert_eq!(prefix.len(), 3);
    assert_eq!(prefix.get(0), Some(&8));
    assert_eq!(prefix.get(2), Some(&1));
    assert!(!prefix.floor_is_empty());
    assert_eq!(prefix.popn(2), 0);
    prefix.push(6);

    let actual: BTreeMap<_, _> = prefix
        .into_gss()
        .to_stacks(8)
        .unwrap()
        .into_iter()
        .collect();
    let expected = BTreeMap::from([(vec![0, 1, 6], Bits(1)), (vec![9, 1, 6], Bits(1))]);
    assert_eq!(actual, expected);
}

#[test]
fn language_keys_ignore_weights_and_layout() {
    let left = WeightedGss::from_stacks([([0_u8, 1, 2], Bits(1)), ([0_u8, 1, 3], Bits(2))]);
    let right = WeightedGss::from_stack([0_u8, 1, 3], Bits(8))
        .merge(&WeightedGss::from_stack([0_u8, 1, 2], Bits(16)));
    let different = WeightedGss::from_stack([0_u8, 1, 4], Bits(1));

    let mut interner = StackLanguageInterner::new();
    assert_eq!(interner.key(&left), interner.key(&right));
    assert_ne!(interner.key(&left), interner.key(&different));
}
