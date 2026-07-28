#![allow(dead_code)]

use rustc_hash::{FxHashMap, FxHashSet};
use std::hash::Hash;
use weighted_gss::{Weight, WeightedGss};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Bits(pub u64);

pub type Entries = Vec<(Vec<u16>, Bits)>;

impl Weight for Bits {
    #[inline]
    fn join(&self, other: &Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Explicit {
    stacks: FxHashMap<Vec<u16>, Bits>,
}

impl Explicit {
    pub fn from_entries(entries: impl IntoIterator<Item = (Vec<u16>, Bits)>) -> Self {
        let entries = entries.into_iter();
        let mut stacks: FxHashMap<Vec<u16>, Bits> = FxHashMap::default();
        stacks.reserve(entries.size_hint().0);
        for (stack, weight) in entries {
            stacks
                .entry(stack)
                .and_modify(|current| *current = current.join(&weight))
                .or_insert(weight);
        }
        Self { stacks }
    }

    pub fn merge(&self, other: &Self) -> Self {
        let (base, added) = if self.stacks.len() >= other.stacks.len() {
            (&self.stacks, &other.stacks)
        } else {
            (&other.stacks, &self.stacks)
        };
        let mut stacks = base.clone();
        stacks.reserve(added.len());
        for (stack, weight) in added {
            stacks
                .entry(stack.clone())
                .and_modify(|current| *current = current.join(weight))
                .or_insert(*weight);
        }
        Self { stacks }
    }

    pub fn push(&self, value: u16) -> Self {
        Self::from_entries(self.stacks.iter().map(|(stack, weight)| {
            let mut next = stack.clone();
            next.push(value);
            (next, *weight)
        }))
    }

    pub fn popn(&self, count: usize) -> Self {
        Self::from_entries(self.stacks.iter().filter_map(|(stack, weight)| {
            let next_len = stack.len().checked_sub(count)?;
            Some((stack[..next_len].to_vec(), *weight))
        }))
    }

    pub fn retain_top(&self, top: u16) -> Self {
        Self::from_entries(
            self.stacks
                .iter()
                .filter(|(stack, _)| stack.last() == Some(&top))
                .map(|(stack, weight)| (stack.clone(), *weight)),
        )
    }

    pub fn map_weights(&self, mut map: impl FnMut(Bits) -> Bits) -> Self {
        Self::from_entries(
            self.stacks
                .iter()
                .map(|(stack, weight)| (stack.clone(), map(*weight))),
        )
    }

    pub fn snapshot(&self) -> Entries {
        self.stacks
            .iter()
            .map(|(stack, weight)| (stack.clone(), *weight))
            .collect()
    }

    pub fn visit_bounded(
        &self,
        max_stacks: usize,
        mut visit: impl FnMut(&[u16], Bits),
    ) -> Result<(), ()> {
        if self.stacks.len() > max_stacks {
            return Err(());
        }
        for (stack, weight) in &self.stacks {
            visit(stack, *weight);
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.stacks.len()
    }
}

#[derive(Clone, Debug, Default)]
pub struct WeightPartitioned {
    by_weight: FxHashMap<Bits, FxHashSet<Vec<u16>>>,
}

impl WeightPartitioned {
    pub fn from_entries(entries: impl IntoIterator<Item = (Vec<u16>, Bits)>) -> Self {
        let mut out = Self::default();
        for (stack, weight) in entries {
            out.by_weight.entry(weight).or_default().insert(stack);
        }
        out
    }

    pub fn merge(&self, other: &Self) -> Self {
        let self_len: usize = self.by_weight.values().map(FxHashSet::len).sum();
        let other_len: usize = other.by_weight.values().map(FxHashSet::len).sum();
        let (base, added) = if self_len >= other_len {
            (self, other)
        } else {
            (other, self)
        };
        let mut out = base.clone();
        for (weight, stacks) in &added.by_weight {
            out.by_weight
                .entry(*weight)
                .or_default()
                .extend(stacks.iter().cloned());
        }
        out
    }

    pub fn push(&self, value: u16) -> Self {
        Self {
            by_weight: self
                .by_weight
                .iter()
                .map(|(weight, stacks)| {
                    let transformed = stacks
                        .iter()
                        .map(|stack| {
                            let mut next = stack.clone();
                            next.push(value);
                            next
                        })
                        .collect();
                    (*weight, transformed)
                })
                .collect(),
        }
    }

    pub fn popn(&self, count: usize) -> Self {
        Self {
            by_weight: self
                .by_weight
                .iter()
                .filter_map(|(weight, stacks)| {
                    let transformed: FxHashSet<_> = stacks
                        .iter()
                        .filter_map(|stack| {
                            let next_len = stack.len().checked_sub(count)?;
                            Some(stack[..next_len].to_vec())
                        })
                        .collect();
                    (!transformed.is_empty()).then_some((*weight, transformed))
                })
                .collect(),
        }
    }

    pub fn retain_top(&self, top: u16) -> Self {
        Self {
            by_weight: self
                .by_weight
                .iter()
                .filter_map(|(weight, stacks)| {
                    let retained: FxHashSet<_> = stacks
                        .iter()
                        .filter(|stack| stack.last() == Some(&top))
                        .cloned()
                        .collect();
                    (!retained.is_empty()).then_some((*weight, retained))
                })
                .collect(),
        }
    }

    pub fn materialize(&self) -> Entries {
        let mut out = FxHashMap::<Vec<u16>, Bits>::default();
        for (weight, stacks) in &self.by_weight {
            for stack in stacks {
                out.entry(stack.clone())
                    .and_modify(|current| *current = current.join(weight))
                    .or_insert(*weight);
            }
        }
        out.into_iter().collect()
    }
}

#[derive(Clone, Debug, Default)]
pub struct ExplicitSet {
    stacks: FxHashSet<Vec<u16>>,
}

impl ExplicitSet {
    pub fn from_stacks(stacks: impl IntoIterator<Item = Vec<u16>>) -> Self {
        Self {
            stacks: stacks.into_iter().collect(),
        }
    }

    pub fn merge(&self, other: &Self) -> Self {
        let (base, added) = if self.stacks.len() >= other.stacks.len() {
            (&self.stacks, &other.stacks)
        } else {
            (&other.stacks, &self.stacks)
        };
        let mut stacks = base.clone();
        stacks.reserve(added.len());
        stacks.extend(added.iter().cloned());
        Self { stacks }
    }

    pub fn push(&self, value: u16) -> Self {
        Self::from_stacks(self.stacks.iter().map(|stack| {
            let mut next = stack.clone();
            next.push(value);
            next
        }))
    }

    pub fn popn(&self, count: usize) -> Self {
        Self::from_stacks(self.stacks.iter().filter_map(|stack| {
            let next_len = stack.len().checked_sub(count)?;
            Some(stack[..next_len].to_vec())
        }))
    }

    pub fn snapshot(&self) -> Vec<Vec<u16>> {
        self.stacks.iter().cloned().collect()
    }
}

pub fn linear_stack(depth: usize) -> Vec<u16> {
    (0..depth).map(|value| value as u16).collect()
}

pub fn homogeneous_stacks(count: usize, depth: usize) -> Vec<Vec<u16>> {
    assert!(depth > 0);
    let floor = linear_stack(depth - 1);
    (0..count)
        .map(|branch| {
            let mut stack = floor.clone();
            stack.push(10_000 + branch as u16);
            stack
        })
        .collect()
}

pub fn weighted_stacks(count: usize, depth: usize, distinct_weights: usize) -> Entries {
    assert!(distinct_weights > 0 && distinct_weights <= 63);
    homogeneous_stacks(count, depth)
        .into_iter()
        .enumerate()
        .map(|(index, stack)| (stack, Bits(1_u64 << (index % distinct_weights))))
        .collect()
}

pub fn binary_stacks(levels: usize) -> Vec<Vec<u16>> {
    let mut stacks = vec![Vec::new()];
    for level in 0..levels {
        let mut next = Vec::with_capacity(stacks.len() * 2);
        for stack in stacks {
            let mut left = stack.clone();
            left.push((level * 2) as u16);
            next.push(left);
            let mut right = stack;
            right.push((level * 2 + 1) as u16);
            next.push(right);
        }
        stacks = next;
    }
    stacks
}

pub fn structurally_build_binary_gss(levels: usize) -> WeightedGss<u16, Bits> {
    let mut value = WeightedGss::from_stack(std::iter::empty(), Bits(1));
    for level in 0..levels {
        value = value
            .push((level * 2) as u16)
            .merge(&value.push((level * 2 + 1) as u16));
    }
    value
}

pub fn structurally_build_binary_explicit(levels: usize) -> Explicit {
    let mut value = Explicit::from_entries([(Vec::new(), Bits(1))]);
    for level in 0..levels {
        value = value
            .push((level * 2) as u16)
            .merge(&value.push((level * 2 + 1) as u16));
    }
    value
}

pub fn structurally_build_two_weight_gss(levels: usize) -> WeightedGss<u16, Bits> {
    if levels == 0 {
        return WeightedGss::from_stack(std::iter::empty(), Bits(1));
    }
    let mut value =
        WeightedGss::from_stack([0_u16], Bits(1)).merge(&WeightedGss::from_stack([1_u16], Bits(2)));
    for level in 1..levels {
        value = value
            .push((level * 2) as u16)
            .merge(&value.push((level * 2 + 1) as u16));
    }
    value
}

pub fn structurally_build_two_weight_explicit(levels: usize) -> Explicit {
    if levels == 0 {
        return Explicit::from_entries([(Vec::new(), Bits(1))]);
    }
    let mut value = Explicit::from_entries([(vec![0_u16], Bits(1)), (vec![1_u16], Bits(2))]);
    for level in 1..levels {
        value = value
            .push((level * 2) as u16)
            .merge(&value.push((level * 2 + 1) as u16));
    }
    value
}

pub fn structurally_build_binary_unweighted_gss(levels: usize) -> weighted_gss::Gss<u16> {
    let mut value = weighted_gss::Gss::from_stack(std::iter::empty(), ());
    for level in 0..levels {
        value = value
            .push((level * 2) as u16)
            .merge(&value.push((level * 2 + 1) as u16));
    }
    value
}

pub fn structurally_build_binary_explicit_set(levels: usize) -> ExplicitSet {
    let mut value = ExplicitSet::from_stacks([Vec::new()]);
    for level in 0..levels {
        value = value
            .push((level * 2) as u16)
            .merge(&value.push((level * 2 + 1) as u16));
    }
    value
}

pub fn top_first_stack_checksum(stack: &[u16], weight: Bits) -> u64 {
    stack.iter().fold(
        weight.0.wrapping_add(stack.len() as u64),
        |checksum, symbol| checksum.wrapping_mul(131).wrapping_add(u64::from(*symbol)),
    )
}

pub fn bottom_first_stack_checksum(stack: &[u16], weight: Bits) -> u64 {
    stack.iter().rev().fold(
        weight.0.wrapping_add(stack.len() as u64),
        |checksum, symbol| checksum.wrapping_mul(131).wrapping_add(u64::from(*symbol)),
    )
}

pub fn overlapping_entries(count: usize, depth: usize) -> (Entries, Entries) {
    let all = homogeneous_stacks(count + count / 2, depth);
    let left = all[..count]
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, stack)| (stack, Bits(1_u64 << (index % 32))))
        .collect();
    let right = all[count / 2..]
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, stack)| (stack, Bits(1_u64 << ((index + 11) % 32))))
        .collect();
    (left, right)
}

pub fn weighted_gss(entries: &Entries) -> WeightedGss<u16, Bits> {
    WeightedGss::from_stacks(entries.iter().cloned())
}
