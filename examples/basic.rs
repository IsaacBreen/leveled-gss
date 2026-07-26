use std::collections::BTreeSet;
use weighted_gss::{StackOp, Weight, WeightedGss};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bits(u32);

impl Weight for Bits {
    fn join(&self, other: &Self) -> Self {
        Self(self.0 | other.0)
    }
}

fn main() {
    let stacks =
        WeightedGss::from_stacks([([0_u32, 1, 2], Bits(0b001)), ([0_u32, 1, 3], Bits(0b100))]);

    assert_eq!(stacks.tops().collect::<BTreeSet<_>>(), [2, 3].into());

    let next = stacks.apply_top_ops([(2, StackOp::new(1, [8])), (3, StackOp::new(0, [9]))]);

    let mut concrete = next.to_stacks(8).unwrap();
    concrete.sort_by(|left, right| left.0.cmp(&right.0));
    println!("{concrete:#?}");
}
