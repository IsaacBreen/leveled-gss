use std::collections::BTreeSet;
use weighted_gss::{Gss, Weight, WeightedGss};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bits(u32);

impl Weight for Bits {
    fn join(&self, other: &Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[test]
fn public_api_reads_like_weighted_stack_operations() {
    let left = WeightedGss::from_stack([0_u8, 1, 2], Bits(1));
    let right = WeightedGss::from_stack([0_u8, 1, 3], Bits(2));
    let stacks = left.merge(&right);

    assert_eq!(
        stacks.tops().collect::<BTreeSet<_>>(),
        BTreeSet::from([2, 3])
    );
    assert_eq!(stacks.top(), None);
    assert!(stacks.retain_empty().is_empty());

    let branch = stacks.pop_top(&2).push(8);
    assert_eq!(branch.top(), Some(8));
    assert_eq!(branch.to_stacks(8).unwrap(), vec![(vec![0, 1, 8], Bits(1))]);
}

#[test]
fn unweighted_alias_is_usable() {
    let stacks = Gss::from_stacks([([0_u8, 1], ()), ([0_u8, 2], ())]);
    assert_eq!(
        stacks.tops().collect::<BTreeSet<_>>(),
        BTreeSet::from([1, 2])
    );
}
