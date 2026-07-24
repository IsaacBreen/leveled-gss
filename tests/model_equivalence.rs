use std::collections::BTreeMap;

use leveled_gss::{LeveledGSS, Merge};
use rand::{Rng, SeedableRng, rngs::StdRng};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Acc(u64);

impl Merge for Acc {
    fn merge(&self, other: &Self) -> Self {
        Self(self.0 | other.0)
    }
}

type Model = BTreeMap<Vec<u8>, Acc>;

fn canonical(stacks: impl IntoIterator<Item = (Vec<u8>, Acc)>) -> Model {
    let mut model = Model::new();
    for (stack, acc) in stacks {
        model
            .entry(stack)
            .and_modify(|current| *current = current.merge(&acc))
            .or_insert(acc);
    }
    model
}

fn materialize(gss: &LeveledGSS<u8, Acc>) -> Model {
    canonical(
        gss.to_stacks(1_000_000)
            .expect("random model unexpectedly exceeded traversal limit"),
    )
}

fn random_model(rng: &mut StdRng) -> Model {
    let count = rng.gen_range(0..=24);
    canonical((0..count).map(|_| {
        let len = rng.gen_range(0..=8);
        let stack = (0..len).map(|_| rng.gen_range(0..=7)).collect();
        let acc = Acc(1_u64 << rng.gen_range(0..16));
        (stack, acc)
    }))
}

fn from_model(model: &Model) -> LeveledGSS<u8, Acc> {
    LeveledGSS::from_stacks(
        &model
            .iter()
            .map(|(stack, acc)| (stack.clone(), *acc))
            .collect::<Vec<_>>(),
    )
}

fn push_model(model: &Model, value: u8) -> Model {
    canonical(model.iter().map(|(stack, acc)| {
        let mut stack = stack.clone();
        stack.push(value);
        (stack, *acc)
    }))
}

fn pop_model(model: &Model, count: usize) -> Model {
    canonical(
        model
            .iter()
            .filter(|(stack, _)| stack.len() >= count)
            .map(|(stack, acc)| {
                let mut stack = stack.clone();
                stack.truncate(stack.len() - count);
                (stack, *acc)
            }),
    )
}

fn isolate_model(model: &Model, value: Option<u8>) -> Model {
    canonical(model.iter().filter_map(|(stack, acc)| match value {
        Some(value) if stack.last() == Some(&value) => Some((stack.clone(), *acc)),
        None if stack.is_empty() => Some((stack.clone(), *acc)),
        _ => None,
    }))
}

fn assert_matches(gss: &LeveledGSS<u8, Acc>, model: &Model, context: &str) {
    assert_eq!(materialize(gss), *model, "semantic mismatch: {context}");
    assert_eq!(gss.is_empty(), model.is_empty());
    assert_eq!(
        gss.max_depth() as usize,
        model.keys().map(Vec::len).max().unwrap_or(0)
    );
    assert_eq!(
        gss.peek()
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>(),
        model
            .keys()
            .filter_map(|stack| stack.last().copied())
            .collect()
    );
    let raw = gss.to_stacks(1_000_000).unwrap();
    assert_eq!(
        gss.path_count_at_most(1_000_000),
        raw.len(),
        "counter/raw traversal mismatch: {context}"
    );
    let expected_acc = model
        .values()
        .copied()
        .reduce(|left, right| left.merge(&right));
    assert_eq!(gss.reduce_acc(), expected_acc);
}

#[test]
fn randomized_operations_match_an_explicit_set_of_stacks() {
    for seed in 0..200_u64 {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut model = random_model(&mut rng);
        let mut gss = from_model(&model);
        assert_matches(&gss, &model, &format!("seed={seed} initial"));

        for step in 0..200 {
            match rng.gen_range(0..6) {
                0 => {
                    let value = rng.gen_range(0..=7);
                    model = push_model(&model, value);
                    gss = gss.push(value);
                }
                1 => {
                    let count = rng.gen_range(0..=5);
                    model = pop_model(&model, count);
                    gss = gss.popn(count as isize);
                }
                2 => {
                    let value = if rng.gen_bool(0.2) {
                        None
                    } else {
                        Some(rng.gen_range(0..=7))
                    };
                    model = isolate_model(&model, value);
                    gss = gss.isolate(value);
                }
                3 => {
                    let other = random_model(&mut rng);
                    model = canonical(model.into_iter().chain(other.clone()));
                    gss = gss.merge(&from_model(&other));
                }
                4 => {
                    let levels = if rng.gen_bool(0.3) {
                        None
                    } else {
                        Some(rng.gen_range(0..=5))
                    };
                    gss = gss.fuse(levels);
                }
                _ => {
                    let rebuilt = from_model(&model);
                    gss = LeveledGSS::merge_many([gss, rebuilt]);
                }
            }
            assert_matches(&gss, &model, &format!("seed={seed} step={step}"));
        }
    }
}

#[test]
fn compressed_binary_dag_preserves_all_paths() {
    let mut gss = LeveledGSS::from_single_stack(Vec::<u8>::new(), Acc(1));
    for level in 0..18 {
        gss = LeveledGSS::merge_many([gss.push(level * 2), gss.push(level * 2 + 1)]);
    }
    assert_eq!(gss.path_count_at_most(300_000), 1 << 18);
    assert!(gss.to_stacks(100_000).is_none());
    assert_eq!(gss.to_stacks(1 << 18).unwrap().len(), 1 << 18);
    assert!(gss.summary().total_unique_nodes < 500);
}

#[test]
fn popping_discards_underflowing_paths() {
    let gss =
        LeveledGSS::from_stacks(&[(vec![], Acc(1)), (vec![10], Acc(2)), (vec![10, 20], Acc(4))]);

    assert_eq!(
        materialize(&gss.popn(1)),
        canonical([(vec![], Acc(2)), (vec![10], Acc(4))])
    );
    assert_eq!(materialize(&gss.popn(2)), canonical([(vec![], Acc(4))]));
    assert!(gss.popn(3).is_empty());
}
