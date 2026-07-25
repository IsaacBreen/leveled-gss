use std::collections::BTreeSet;
use weighted_gss::{Gss, StackEffect, Weight, WeightedGss};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bits(u32);

impl Weight for Bits {
    fn join(&self, other: &Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[test]
fn public_api_reads_like_stack_operations() {
    let left = WeightedGss::from_stack([0_u8, 1, 2], Bits(1));
    let right = WeightedGss::from_stack([0_u8, 1, 3], Bits(2));
    let stacks = left.merge(&right);

    assert_eq!(
        stacks.tops().collect::<BTreeSet<_>>(),
        BTreeSet::from([2, 3])
    );
    assert_eq!(stacks.top(), None);

    let branch = stacks.pop_top(&2);
    assert_eq!(branch.top(), Some(1));
    assert_eq!(branch.to_stacks(8).unwrap(), vec![(vec![0, 1], Bits(1))]);

    let shifted =
        stacks.apply_top_effects([(2, StackEffect::new(1, [8])), (3, StackEffect::new(0, [9]))]);
    let mut materialized = shifted.to_stacks(8).unwrap();
    materialized.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        materialized,
        vec![(vec![0, 1, 3, 9], Bits(2)), (vec![0, 1, 8], Bits(1)),]
    );
}

#[test]
fn unweighted_alias_is_usable() {
    let stacks = Gss::from_stacks([([0_u8, 1], ()), ([0_u8, 2], ())]);
    assert_eq!(
        stacks.tops().collect::<BTreeSet<_>>(),
        BTreeSet::from([1, 2])
    );
}
