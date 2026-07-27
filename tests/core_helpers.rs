use std::collections::BTreeMap;
use weighted_gss::{Weight, WeightedGss, for_each_stack_top_first, linear_prefix};

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
    assert_eq!(
        error.to_string(),
        "the weighted GSS exceeds the configured distinct-stack limit"
    );
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
fn bounded_visit_handles_a_deep_single_stack_iteratively() {
    let mut gss = WeightedGss::from_stack(Vec::<u32>::new(), Bits(1));
    for symbol in 0..20_000_u32 {
        gss = gss.push(symbol);
    }

    let mut seen = 0;
    for_each_stack_top_first(&gss, 1, |stack, weight| {
        assert_eq!(stack.len(), 20_000);
        assert_eq!(stack.first(), Some(&19_999));
        assert_eq!(stack.last(), Some(&0));
        assert_eq!(*weight, Bits(1));
        seen += 1;
    })
    .unwrap();
    assert_eq!(seen, 1);

    let [(stack, weight)] = gss.to_stacks(1).unwrap().try_into().unwrap();
    assert_eq!(stack.len(), 20_000);
    assert_eq!(stack.first(), Some(&0));
    assert_eq!(stack.last(), Some(&19_999));
    assert_eq!(weight, Bits(1));

    // Dropping an artificially deep Arc chain is independently recursive.
    std::mem::forget(gss);
}
