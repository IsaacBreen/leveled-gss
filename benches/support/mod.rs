#![allow(dead_code)]

use rustc_hash::{FxHashMap, FxHashSet};
use std::hash::Hash;
use weighted_gss::{Weight, WeightedGss};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
        let mut out = Self::default();
        for (stack, weight) in entries {
            out.stacks
                .entry(stack)
                .and_modify(|current| *current = current.join(&weight))
                .or_insert(weight);
        }
        out
    }

    pub fn merge(&self, other: &Self) -> Self {
        Self::from_entries(
            self.stacks
                .iter()
                .chain(&other.stacks)
                .map(|(stack, weight)| (stack.clone(), *weight)),
        )
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

    pub fn snapshot(&self) -> Entries {
        self.stacks
            .iter()
            .map(|(stack, weight)| (stack.clone(), *weight))
            .collect()
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
        let mut out = self.clone();
        for (weight, stacks) in &other.by_weight {
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
        Self {
            stacks: self.stacks.union(&other.stacks).cloned().collect(),
        }
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
        .map(|(index, stack)| (stack, Bits(1_u64 << ((index + 7) % 32))))
        .collect();
    (left, right)
}

pub fn weighted_gss(entries: &[(Vec<u16>, Bits)]) -> WeightedGss<u16, Bits> {
    WeightedGss::from_stacks(entries.iter().cloned())
}
