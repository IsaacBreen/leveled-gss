use weighted_gss::{Weight, WeightedGss};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Bits(u8);

impl Weight for Bits {
    fn join(&self, other: &Self) -> Self {
        Self(self.0 | other.0)
    }
}

fn main() {
    let stacks =
        WeightedGss::from_stacks([([0_u32, 1, 2], Bits(0b001)), ([0_u32, 1, 3], Bits(0b100))]);

    let next = stacks.pop_top(&2).push(8);
    println!("{next:?}");
}
