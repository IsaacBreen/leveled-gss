use rand::{Rng, SeedableRng, rngs::StdRng};
use std::collections::{BTreeMap, BTreeSet};
use weighted_gss::{StackOp, Weight, WeightedGss};

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

fn retain_empty_model(model: &Model) -> Model {
    canonical(
        model
            .iter()
            .filter(|(stack, _)| stack.is_empty())
            .map(|(stack, weight)| (stack.clone(), *weight)),
    )
}

fn retain_depth_model(model: &Model, depth: usize, parity: u8) -> Model {
    canonical(model.iter().filter_map(|(stack, weight)| {
        let index = stack.len().checked_sub(depth + 1)?;
        (stack[index] % 2 == parity).then(|| (stack.clone(), *weight))
    }))
}

fn effect_model(model: &Model, pop: usize, push: &[u8]) -> Model {
    canonical(model.iter().filter_map(|(stack, weight)| {
        if stack.len() < pop {
            return None;
        }
        let mut next = stack[..stack.len() - pop].to_vec();
        next.extend_from_slice(push);
        Some((next, *weight))
    }))
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
    assert_eq!(gss.paths().path_count_at_most(10), 1);
    assert_eq!(gss.to_stacks(4).unwrap(), vec![(vec![1, 2], Bits(5))]);
}

#[test]
fn path_local_weight_transforms_preserve_stack_correlation() {
    let gss = WeightedGss::from_stacks([
        (vec![1_u8, 2], Bits(1)),
        (vec![1_u8, 3], Bits(4)),
        (vec![9_u8], Bits(8)),
    ]);
    let mapped = gss
        .paths()
        .filter_map_weights(|weight| (weight.0 != 4).then_some(Bits(weight.0 << 1)));
    let mut raw = mapped.to_stacks(8).unwrap();
    raw.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(raw, vec![(vec![1, 2], Bits(2)), (vec![9], Bits(16))]);
}

#[test]
fn virtual_stack_exposes_linear_prefix_over_hidden_floor() {
    let base = WeightedGss::from_stack(Vec::<u8>::new(), Bits(1)).apply_ops([
        StackOp::new(0, vec![0_u8, 1]),
        StackOp::new(0, vec![9_u8, 1]),
    ]);
    let pushed = base.push(7).push(8);
    let mut virtual_stack = pushed.try_virtual_stack().expect("linear prefix");
    assert_eq!(virtual_stack.top(), Some(&8));
    assert_eq!(virtual_stack.get_from_top(1), Some(&7));
    assert_eq!(virtual_stack.prefix_len(), 3); // 8, 7, and shared 1
    assert!(!virtual_stack.is_complete());
    assert_eq!(virtual_stack.pop_prefix(2), 0);
    assert_eq!(virtual_stack.top(), Some(&1));
    assert_eq!(virtual_stack.pop_prefix(2), 1); // one pop reaches the hidden floor
    let remainder = virtual_stack.into_gss();
    assert_eq!(
        materialize(&remainder),
        canonical([(vec![0], Bits(1)), (vec![9], Bits(1))])
    );
}

#[test]
fn top_branching_and_depth_filters_are_extensional() {
    let gss = WeightedGss::from_stacks([
        (vec![0_u8, 1, 2], Bits(1)),
        (vec![0_u8, 3, 2], Bits(2)),
        (vec![9_u8, 4], Bits(4)),
        (Vec::<u8>::new(), Bits(8)),
    ]);
    assert_eq!(gss.top(), None);
    assert_eq!(gss.pop_branches().len(), 2);
    assert_eq!(
        materialize(&gss.pop_top(&2)),
        canonical([(vec![0, 1], Bits(1)), (vec![0, 3], Bits(2))])
    );
    assert_eq!(
        materialize(&gss.retain_where_at_depth(1, |value| *value % 2 == 1)),
        canonical([
            (vec![0, 1, 2], Bits(1)),
            (vec![0, 3, 2], Bits(2)),
            (vec![9, 4], Bits(4)),
        ])
    );
    assert_eq!(gss.empty_weight(), Some(Bits(8)));
}

#[test]
fn randomized_core_operations_match_extensional_model() {
    for seed in 0..100_u64 {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut model = Model::new();
        let mut gss = WeightedGss::new();

        for step in 0..150 {
            match rng.gen_range(0..9) {
                0 => {
                    let len = rng.gen_range(0..=7);
                    let stack: Vec<u8> = (0..len).map(|_| rng.gen_range(0..=5)).collect();
                    let weight = Bits(1 << rng.gen_range(0..16));
                    model = canonical(model.into_iter().chain([(stack.clone(), weight)]));
                    gss = gss.with_stack(stack, weight);
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
                    model = retain_empty_model(&model);
                    gss = gss.retain_empty();
                }
                5 => {
                    let depth = rng.gen_range(0..=4);
                    let parity = rng.gen_range(0..=1);
                    model = retain_depth_model(&model, depth, parity);
                    gss = gss.retain_where_at_depth(depth, |value| *value % 2 == parity);
                }
                6 => {
                    let count = rng.gen_range(0..=5);
                    let push_len = rng.gen_range(0..=3);
                    let push: Vec<u8> = (0..push_len).map(|_| rng.gen_range(0..=5)).collect();
                    model = effect_model(&model, count, &push);
                    gss = gss.apply_op(StackOp::new(count, push));
                }
                7 => {
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
                    if let Some(top) = model.keys().filter_map(|s| s.last()).copied().next() {
                        model = pop_model(&retain_top_model(&model, top), 1);
                        gss = gss.pop_top(&top);
                    }
                }
            }
            assert_matches(&gss, &model, &format!("seed={seed} step={step}"));
        }
    }
}

#[test]
fn stack_language_interner_ignores_weights_and_layout() {
    use weighted_gss::StackLanguageInterner;

    let canonical =
        WeightedGss::from_stacks([(vec![0_u8, 1, 2], Bits(1)), (vec![0_u8, 1, 3], Bits(2))]);
    let reversed = WeightedGss::merge_all([
        WeightedGss::from_stack([0_u8, 1, 3], Bits(64)),
        WeightedGss::from_stack([0_u8, 1, 2], Bits(32)),
    ]);
    let different = WeightedGss::from_stack([0_u8, 1, 4], Bits(1));

    let mut interner = StackLanguageInterner::new();
    assert_eq!(interner.key(&canonical), interner.key(&reversed));
    assert_ne!(interner.key(&canonical), interner.key(&different));
}

#[test]
fn stack_language_interner_stays_compact_for_shared_binary_dag() {
    use weighted_gss::StackLanguageInterner;

    let mut gss = WeightedGss::from_stack(Vec::<u8>::new(), Bits(1));
    for level in 0..18_u8 {
        gss = WeightedGss::merge_all([gss.push(level * 2), gss.push(level * 2 + 1)]);
    }
    assert_eq!(gss.paths().path_count_at_most(1 << 19), 1 << 18);
    let mut interner = StackLanguageInterner::new();
    assert_ne!(interner.key(&gss).as_u32(), 0);
    assert!(interner.node_count() < 100);
}

#[test]
fn homogeneous_constructor_preserves_common_linear_prefix() {
    let gss = WeightedGss::from_stacks_with_weight([vec![0_u8, 1, 7], vec![9_u8, 1, 7]], Bits(1));
    let virtual_stack = gss.try_virtual_stack().expect("common linear prefix");
    assert_eq!(virtual_stack.top(), Some(&7));
    assert_eq!(virtual_stack.get_from_top(1), Some(&1));
    assert_eq!(virtual_stack.prefix_len(), 2);
    assert!(!virtual_stack.is_complete());
}

#[test]
fn bounded_path_visit_emits_limit_then_reports_overflow() {
    let gss = WeightedGss::from_stacks_with_weight([vec![0_u8], vec![1_u8], vec![2_u8]], Bits(1));
    let mut visited = Vec::new();
    let result = gss
        .paths()
        .for_each_path_top_first(2, |stack, _| visited.push(stack.to_vec()));
    assert_eq!(visited.len(), 2);
    assert_eq!(result.unwrap_err().limit, 2);
}

#[test]
fn stack_language_interner_is_safe_across_dropped_frontiers() {
    use weighted_gss::StackLanguageInterner;

    let mut interner = StackLanguageInterner::new();
    let mut ids = std::collections::BTreeSet::new();
    for symbol in 0_u16..1_000 {
        let frontier = WeightedGss::from_stack([symbol], Bits(1));
        assert!(ids.insert(interner.key(&frontier).as_u32()));
    }
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
