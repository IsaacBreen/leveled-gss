#![no_main]

use libfuzzer_sys::fuzz_target;
use std::collections::{BTreeMap, BTreeSet};
use weighted_gss::{Weight, WeightedGss, for_each_stack_top_first, linear_prefix};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
struct Bits(u64);

impl Weight for Bits {
    fn join(&self, other: &Self) -> Self {
        Self(self.0 | other.0)
    }
}

type Model = BTreeMap<Vec<u8>, Bits>;

struct Input<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Input<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn next(&mut self) -> u8 {
        let value = self.bytes.get(self.cursor).copied().unwrap_or(0);
        self.cursor = self.cursor.saturating_add(1);
        value
    }

    fn stack(&mut self) -> Vec<u8> {
        let len = usize::from(self.next() % 9);
        (0..len).map(|_| self.next() % 8).collect()
    }

    fn weight(&mut self) -> Bits {
        let low = u64::from(self.next());
        let high = u64::from(self.next());
        Bits(low | (high << 8))
    }
}

fn canonical(entries: impl IntoIterator<Item = (Vec<u8>, Bits)>) -> Model {
    let mut out = Model::new();
    for (stack, weight) in entries {
        out.entry(stack)
            .and_modify(|current| *current = current.join(&weight))
            .or_insert(weight);
    }
    out
}

fn from_model(model: &Model) -> WeightedGss<u8, Bits> {
    WeightedGss::from_stacks(model.iter().map(|(stack, weight)| (stack.clone(), *weight)))
}

fn push(model: &Model, value: u8) -> Model {
    canonical(model.iter().map(|(stack, weight)| {
        let mut next = stack.clone();
        next.push(value);
        (next, *weight)
    }))
}

fn popn(model: &Model, count: usize) -> Model {
    canonical(model.iter().filter_map(|(stack, weight)| {
        let next_len = stack.len().checked_sub(count)?;
        Some((stack[..next_len].to_vec(), *weight))
    }))
}

fn retain_top(model: &Model, top: u8) -> Model {
    canonical(
        model
            .iter()
            .filter(|(stack, _)| stack.last() == Some(&top))
            .map(|(stack, weight)| (stack.clone(), *weight)),
    )
}

fn retain_empty(model: &Model) -> Model {
    model
        .get(&Vec::new())
        .copied()
        .map(|weight| BTreeMap::from([(Vec::new(), weight)]))
        .unwrap_or_default()
}

fn materialize(gss: &WeightedGss<u8, Bits>, limit: usize) -> Model {
    canonical(gss.to_stacks(limit).expect("fuzz model bound"))
}

fn assert_matches(gss: &WeightedGss<u8, Bits>, model: &Model, step: usize) {
    let limit = model.len().max(1);
    assert_eq!(materialize(gss, limit), *model, "semantic mismatch at {step}");
    assert_eq!(gss.is_empty(), model.is_empty(), "empty mismatch at {step}");
    assert_eq!(
        gss.max_depth(),
        model.keys().map(Vec::len).max().unwrap_or(0),
        "depth mismatch at {step}"
    );
    assert_eq!(
        gss.tops().collect::<BTreeSet<_>>(),
        model
            .keys()
            .filter_map(|stack| stack.last().copied())
            .collect(),
        "tops mismatch at {step}"
    );
    assert_eq!(
        gss.has_empty_stack(),
        model.contains_key(&Vec::new()),
        "empty-stack mismatch at {step}"
    );
    assert_eq!(
        gss.joined_weight(),
        model.values().copied().reduce(|a, b| a.join(&b)),
        "joined-weight mismatch at {step}"
    );

    let mut visited = Vec::new();
    for_each_stack_top_first(gss, model.len(), |stack, weight| {
        visited.push((stack.iter().rev().copied().collect(), *weight));
    })
    .expect("exact visitor bound");
    assert_eq!(canonical(visited), *model, "visitor mismatch at {step}");

    if !model.is_empty() {
        let mut callbacks = 0;
        assert!(
            for_each_stack_top_first(gss, model.len() - 1, |_, _| callbacks += 1).is_err()
        );
        assert_eq!(callbacks, 0, "visitor emitted before overflow at {step}");
    }

    if let Some(prefix) = linear_prefix(gss) {
        assert_eq!(
            materialize(&prefix.into_gss(), limit),
            *model,
            "linear-prefix round trip at {step}"
        );
    }
}

fuzz_target!(|data: &[u8]| {
    let mut input = Input::new(data);
    let mut gss = WeightedGss::new();
    let mut model = Model::new();
    let mut snapshots = vec![(gss.clone(), model.clone())];

    for step in 0..data.len().min(512) {
        let old_gss = gss.clone();
        let old_model = model.clone();
        match input.next() % 10 {
            0 if model.len() < 256 => {
                let stack = input.stack();
                let weight = input.weight();
                gss = gss.merge(&WeightedGss::from_stack(stack.clone(), weight));
                model = canonical(model.into_iter().chain([(stack, weight)]));
            }
            1 if model.len() < 256 => {
                let count = usize::from(input.next() % 5);
                let entries: Vec<_> = (0..count)
                    .map(|_| (input.stack(), input.weight()))
                    .collect();
                let other = canonical(entries);
                gss = gss.merge(&from_model(&other));
                model = canonical(model.into_iter().chain(other));
            }
            2 => {
                let value = input.next() % 8;
                gss = gss.push(value);
                model = push(&model, value);
            }
            3 => {
                let count = usize::from(input.next() % 9);
                gss = gss.popn(count);
                model = popn(&model, count);
            }
            4 => {
                let top = input.next() % 8;
                gss = gss.retain_top(&top);
                model = retain_top(&model, top);
            }
            5 => {
                let top = input.next() % 8;
                gss = gss.pop_top(&top);
                model = popn(&retain_top(&model, top), 1);
            }
            6 => {
                gss = gss.retain_empty();
                model = retain_empty(&model);
            }
            7 => {
                let mask = u64::from(input.next()) | (u64::from(input.next()) << 8);
                gss = gss.map_weights(|weight| Bits(weight.0 & mask));
                model = canonical(
                    model
                        .iter()
                        .map(|(stack, weight)| (stack.clone(), Bits(weight.0 & mask))),
                );
            }
            8 => {
                let mask = u64::from(input.next()) | (u64::from(input.next()) << 8);
                gss = gss.filter_map_weights(|weight| {
                    let masked = weight.0 & mask;
                    (masked != 0).then_some(Bits(masked))
                });
                model = canonical(model.iter().filter_map(|(stack, weight)| {
                    let masked = weight.0 & mask;
                    (masked != 0).then(|| (stack.clone(), Bits(masked)))
                }));
            }
            _ => {
                let selected = usize::from(input.next()) % snapshots.len();
                (gss, model) = snapshots[selected].clone();
            }
        }

        assert_matches(&old_gss, &old_model, step);
        assert_matches(&gss, &model, step);
        if snapshots.len() == 16 {
            snapshots.remove(0);
        }
        snapshots.push((gss.clone(), model.clone()));
    }
});
