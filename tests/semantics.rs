use rand::{Rng, SeedableRng, rngs::StdRng};
use std::collections::{BTreeMap, BTreeSet};
use weighted_gss::{Weight, WeightedGss};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Bits(u64);

impl Weight for Bits {
    fn join(&self, other: &Self) -> Self {
        Self(self.0 | other.0)
    }
}

type Model = BTreeMap<Vec<u8>, Bits>;

fn canonical(entries: impl IntoIterator<Item = (Vec<u8>, Bits)>) -> Model {
    let mut out = Model::new();
    for (stack, weight) in entries {
        out.entry(stack)
            .and_modify(|current| *current = current.join(&weight))
            .or_insert(weight);
    }
    out
}

fn materialize(gss: &WeightedGss<u8, Bits>) -> Model {
    canonical(gss.to_stacks(1_000_000).expect("test path limit"))
}

fn from_model(model: &Model) -> WeightedGss<u8, Bits> {
    WeightedGss::from_stacks(model.iter().map(|(stack, weight)| (stack.clone(), *weight)))
}

fn push_model(model: &Model, value: u8) -> Model {
    canonical(model.iter().map(|(stack, weight)| {
        let mut next = stack.clone();
        next.push(value);
        (next, *weight)
    }))
}

fn pop_model(model: &Model, count: usize) -> Model {
    canonical(model.iter().filter_map(|(stack, weight)| {
        if stack.len() < count {
            return None;
        }
        let mut next = stack.clone();
        next.truncate(next.len() - count);
        Some((next, *weight))
    }))
}

fn retain_top_model(model: &Model, top: u8) -> Model {
    canonical(
        model
            .iter()
            .filter(|(stack, _)| stack.last() == Some(&top))
            .map(|(stack, weight)| (stack.clone(), *weight)),
    )
}

fn assert_matches(gss: &WeightedGss<u8, Bits>, model: &Model, context: &str) {
    assert_eq!(materialize(gss), *model, "semantic mismatch: {context}");
    assert_eq!(
        gss.is_empty(),
        model.is_empty(),
        "empty mismatch: {context}"
    );
    assert_eq!(
        gss.max_depth(),
        model.keys().map(Vec::len).max().unwrap_or(0),
        "depth mismatch: {context}"
    );
    assert_eq!(
        gss.tops().collect::<BTreeSet<_>>(),
        model
            .keys()
            .filter_map(|stack| stack.last().copied())
            .collect(),
        "tops mismatch: {context}"
    );
    assert_eq!(
        gss.has_empty_stack(),
        model.contains_key(&Vec::new()),
        "empty-stack mismatch: {context}"
    );
    let expected_join = model.values().copied().reduce(|a, b| a.join(&b));
    assert_eq!(
        gss.joined_weight(),
        expected_join,
        "weight mismatch: {context}"
    );
}

#[test]
fn merge_joins_weights_when_stack_keys_coincide() {
    let gss = WeightedGss::from_stacks([([1_u8, 2], Bits(1)), ([1_u8, 2], Bits(4))]);
    assert_eq!(gss.to_stacks(4).unwrap(), vec![(vec![1, 2], Bits(5))]);
}

#[test]
fn top_selection_and_empty_stack_are_distinct() {
    let gss = WeightedGss::from_stacks([
        (vec![0_u8, 1, 2], Bits(1)),
        (vec![0_u8, 3, 2], Bits(2)),
        (vec![9_u8, 4], Bits(4)),
        (Vec::<u8>::new(), Bits(8)),
    ]);

    assert_eq!(gss.top(), None);
    assert!(gss.has_empty_stack());
    assert_eq!(
        materialize(&gss.pop_top(&2)),
        canonical([(vec![0, 1], Bits(1)), (vec![0, 3], Bits(2))])
    );
    assert!(gss.retain_top(&7).is_empty());
}

#[test]
fn popn_discards_underflowing_alternatives() {
    let gss = WeightedGss::from_stacks([
        (vec![1_u8], Bits(1)),
        (vec![2_u8, 3], Bits(2)),
        (Vec::<u8>::new(), Bits(4)),
    ]);
    assert_eq!(
        materialize(&gss.popn(1)),
        canonical([(Vec::new(), Bits(1)), (vec![2], Bits(2))])
    );
    assert!(gss.popn(3).is_empty());
}

#[test]
fn homogeneous_constructor_has_expected_meaning() {
    let gss = WeightedGss::from_stacks_with_weight([vec![0_u8, 1, 7], vec![9_u8, 1, 7]], Bits(1));
    assert_eq!(
        materialize(&gss),
        canonical([(vec![0, 1, 7], Bits(1)), (vec![9, 1, 7], Bits(1))])
    );
}

#[test]
fn randomized_core_operations_match_extensional_model() {
    for seed in 0..100_u64 {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut model = Model::new();
        let mut gss = WeightedGss::new();

        for step in 0..150 {
            match rng.gen_range(0..7) {
                0 => {
                    let len = rng.gen_range(0..=7);
                    let stack: Vec<u8> = (0..len).map(|_| rng.gen_range(0..=5)).collect();
                    let weight = Bits(1 << rng.gen_range(0..16));
                    model = canonical(model.into_iter().chain([(stack.clone(), weight)]));
                    gss = gss.merge(&WeightedGss::from_stack(stack, weight));
                }
                1 => {
                    let value = rng.gen_range(0..=5);
                    model = push_model(&model, value);
                    gss = gss.push(value);
                }
                2 => {
                    let count = rng.gen_range(0..=5);
                    model = pop_model(&model, count);
                    gss = gss.popn(count);
                }
                3 => {
                    let top = rng.gen_range(0..=5);
                    model = retain_top_model(&model, top);
                    gss = gss.retain_top(&top);
                }
                4 => {
                    if let Some(top) = model
                        .keys()
                        .filter_map(|stack| stack.last())
                        .copied()
                        .next()
                    {
                        model = pop_model(&retain_top_model(&model, top), 1);
                        gss = gss.pop_top(&top);
                    }
                }
                5 => {
                    let other_count = rng.gen_range(0..=8);
                    let mut other = Model::new();
                    for _ in 0..other_count {
                        let len = rng.gen_range(0..=5);
                        let stack = (0..len).map(|_| rng.gen_range(0..=5)).collect::<Vec<_>>();
                        let weight = Bits(1 << rng.gen_range(0..16));
                        other = canonical(other.into_iter().chain([(stack, weight)]));
                    }
                    model = canonical(model.into_iter().chain(other.clone()));
                    gss = gss.merge(&from_model(&other));
                }
                _ => {
                    model = pop_model(&model, 1);
                    gss = gss.pop();
                }
            }
            assert_matches(&gss, &model, &format!("seed={seed} step={step}"));
        }
    }
}

#[test]
fn materialization_limit_is_never_silent() {
    let mut gss = WeightedGss::from_stack(Vec::<u8>::new(), Bits(1));
    for level in 0..12_u8 {
        gss = gss.push(level * 2).merge(&gss.push(level * 2 + 1));
    }
    assert_eq!(gss.to_stacks(100).unwrap_err().limit, 100);
    assert_eq!(gss.to_stacks(1 << 12).unwrap().len(), 1 << 12);
}

#[test]
fn merge_memo_does_not_alias_dropped_intermediate_nodes() {
    let entries = [
        (vec![2_u8, 1, 5, 1, 0], Bits(64)),
        (vec![5], Bits(128)),
        (vec![1, 2], Bits(64)),
        (vec![4, 2, 5, 4, 1], Bits(32)),
        (vec![], Bits(1)),
        (vec![1, 2, 4, 4, 1], Bits(2)),
        (vec![1, 2], Bits(128)),
    ];
    let expected = canonical(entries.clone());

    for _ in 0..1_000 {
        let gss = WeightedGss::from_stacks(entries.clone());
        assert_eq!(materialize(&gss), expected);
    }
}
