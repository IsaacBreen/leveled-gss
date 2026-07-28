mod support;

use proptest::prelude::*;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use support::{
    Bits, Model, assert_matches, canonical, filter_mask, from_model, map_mask, pop_top, popn, push,
    retain_empty, retain_top,
};
use weighted_gss::{Weight, WeightedGss};

#[derive(Clone, Debug)]
enum Op {
    Add(Vec<u8>, Bits),
    Merge(Vec<(Vec<u8>, Bits)>),
    Push(u8),
    PopN(usize),
    RetainTop(u8),
    RetainEmpty,
    PopTop(u8),
    MapMask(u64),
    FilterMask(u64),
    Restore(usize),
}

fn stack_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(0_u8..=7, 0..=8)
}

fn weight_strategy() -> impl Strategy<Value = Bits> {
    any::<u16>().prop_map(|bits| Bits(u64::from(bits)))
}

fn operation_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![
        5 => (stack_strategy(), weight_strategy()).prop_map(|(stack, weight)| Op::Add(stack, weight)),
        3 => prop::collection::vec((stack_strategy(), weight_strategy()), 0..=8).prop_map(Op::Merge),
        4 => (0_u8..=7).prop_map(Op::Push),
        4 => (0_usize..=8).prop_map(Op::PopN),
        3 => (0_u8..=7).prop_map(Op::RetainTop),
        1 => Just(Op::RetainEmpty),
        3 => (0_u8..=7).prop_map(Op::PopTop),
        2 => any::<u16>().prop_map(|mask| Op::MapMask(u64::from(mask))),
        2 => any::<u16>().prop_map(|mask| Op::FilterMask(u64::from(mask))),
        1 => any::<u8>().prop_map(|index| Op::Restore(usize::from(index))),
    ]
}

fn apply(op: &Op, gss: &WeightedGss<u8, Bits>, model: &Model) -> (WeightedGss<u8, Bits>, Model) {
    match op {
        Op::Add(stack, weight) => (
            gss.merge(&WeightedGss::from_stack(stack.clone(), *weight)),
            canonical(model.clone().into_iter().chain([(stack.clone(), *weight)])),
        ),
        Op::Merge(entries) => {
            let other = canonical(entries.clone());
            (
                gss.merge(&from_model(&other)),
                canonical(model.clone().into_iter().chain(other)),
            )
        }
        Op::Push(value) => (gss.push(*value), push(model, *value)),
        Op::PopN(count) => (gss.popn(*count), popn(model, *count)),
        Op::RetainTop(top) => (gss.retain_top(top), retain_top(model, *top)),
        Op::RetainEmpty => (gss.retain_empty(), retain_empty(model)),
        Op::PopTop(top) => (gss.pop_top(top), pop_top(model, *top)),
        Op::MapMask(mask) => (
            gss.map_weights(|weight| Bits(weight.0 & mask)),
            map_mask(model, *mask),
        ),
        Op::FilterMask(mask) => (
            gss.filter_map_weights(|weight| {
                let masked = weight.0 & mask;
                (masked != 0).then_some(Bits(masked))
            }),
            filter_mask(model, *mask),
        ),
        Op::Restore(_) => unreachable!("restore is handled by the state machine"),
    }
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 96,
        max_shrink_iters: 20_000,
        ..ProptestConfig::default()
    })]

    #[test]
    fn shrinkable_operation_sequences_match_the_explicit_model(
        operations in prop::collection::vec(operation_strategy(), 1..=160),
    ) {
        let mut gss = WeightedGss::new();
        let mut model = Model::new();
        let mut snapshots = vec![(gss.clone(), model.clone())];

        for (step, op) in operations.iter().enumerate() {
            let old_gss = gss.clone();
            let old_model = model.clone();

            if let Op::Restore(index) = op {
                let selected = index % snapshots.len();
                (gss, model) = snapshots[selected].clone();
            } else {
                (gss, model) = apply(op, &gss, &model);
            }

            assert_matches(&old_gss, &old_model, &format!("preserved snapshot before step={step}, op={op:?}"));
            assert_matches(&gss, &model, &format!("step={step}, op={op:?}"));

            if snapshots.len() == 24 {
                snapshots.remove(0);
            }
            snapshots.push((gss.clone(), model.clone()));
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct BadHash(u8);

impl Hash for BadHash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        0_u8.hash(state);
    }
}

#[test]
fn constant_hash_symbols_preserve_semantics() {
    let entries = (0_u8..64).map(|value| {
        (
            vec![BadHash(value % 4), BadHash(value / 4), BadHash(value % 7)],
            Bits(1_u64 << (value % 32)),
        )
    });
    let gss = WeightedGss::from_stacks(entries.clone())
        .pop()
        .push(BadHash(9))
        .merge(&WeightedGss::from_stack(
            [BadHash(1), BadHash(2), BadHash(9)],
            Bits(1 << 40),
        ));

    let mut model = BTreeMap::<Vec<BadHash>, Bits>::new();
    for (mut stack, weight) in entries {
        stack.pop();
        stack.push(BadHash(9));
        model
            .entry(stack)
            .and_modify(|current| *current = current.join(&weight))
            .or_insert(weight);
    }
    model
        .entry(vec![BadHash(1), BadHash(2), BadHash(9)])
        .and_modify(|current| *current = current.join(&Bits(1 << 40)))
        .or_insert(Bits(1 << 40));

    let actual: BTreeMap<_, _> = gss.to_stacks(1_000).unwrap().into_iter().collect();
    assert_eq!(actual, model);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Minimum(u8);

impl Weight for Minimum {
    fn join(&self, other: &Self) -> Self {
        Self(self.0.min(other.0))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Maximum(u8);

impl Weight for Maximum {
    fn join(&self, other: &Self) -> Self {
        Self(self.0.max(other.0))
    }
}

#[test]
fn different_valid_join_algebras_collapse_correctly() {
    let min = WeightedGss::from_stacks([
        (vec![0_u8, 1], Minimum(9)),
        (vec![0_u8, 2], Minimum(3)),
        (vec![0_u8, 3], Minimum(7)),
    ])
    .pop();
    assert_eq!(min.to_stacks(1).unwrap(), vec![(vec![0], Minimum(3))]);

    let max = WeightedGss::from_stacks([
        (vec![0_u8, 1], Maximum(9)),
        (vec![0_u8, 2], Maximum(3)),
        (vec![0_u8, 3], Maximum(7)),
    ])
    .pop();
    assert_eq!(max.to_stacks(1).unwrap(), vec![(vec![0], Maximum(9))]);
}
