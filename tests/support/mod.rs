#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use weighted_gss::{Weight, WeightedGss, for_each_stack_top_first, linear_prefix};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Bits(pub u64);

impl Weight for Bits {
    fn join(&self, other: &Self) -> Self {
        Self(self.0 | other.0)
    }
}

pub type Model = BTreeMap<Vec<u8>, Bits>;

pub fn canonical(entries: impl IntoIterator<Item = (Vec<u8>, Bits)>) -> Model {
    let mut out = Model::new();
    for (stack, weight) in entries {
        out.entry(stack)
            .and_modify(|current| *current = current.join(&weight))
            .or_insert(weight);
    }
    out
}

pub fn from_model(model: &Model) -> WeightedGss<u8, Bits> {
    WeightedGss::from_stacks(model.iter().map(|(stack, weight)| (stack.clone(), *weight)))
}

pub fn materialize(gss: &WeightedGss<u8, Bits>) -> Model {
    canonical(gss.to_stacks(1_000_000).expect("property-test stack bound"))
}

pub fn push(model: &Model, value: u8) -> Model {
    canonical(model.iter().map(|(stack, weight)| {
        let mut next = stack.clone();
        next.push(value);
        (next, *weight)
    }))
}

pub fn popn(model: &Model, count: usize) -> Model {
    canonical(model.iter().filter_map(|(stack, weight)| {
        let next_len = stack.len().checked_sub(count)?;
        Some((stack[..next_len].to_vec(), *weight))
    }))
}

pub fn retain_top(model: &Model, top: u8) -> Model {
    canonical(
        model
            .iter()
            .filter(|(stack, _)| stack.last() == Some(&top))
            .map(|(stack, weight)| (stack.clone(), *weight)),
    )
}

pub fn retain_empty(model: &Model) -> Model {
    model
        .get(&Vec::new())
        .copied()
        .map(|weight| BTreeMap::from([(Vec::new(), weight)]))
        .unwrap_or_default()
}

pub fn pop_top(model: &Model, top: u8) -> Model {
    popn(&retain_top(model, top), 1)
}

pub fn map_mask(model: &Model, mask: u64) -> Model {
    canonical(
        model
            .iter()
            .map(|(stack, weight)| (stack.clone(), Bits(weight.0 & mask))),
    )
}

pub fn filter_mask(model: &Model, mask: u64) -> Model {
    canonical(model.iter().filter_map(|(stack, weight)| {
        let masked = weight.0 & mask;
        (masked != 0).then(|| (stack.clone(), Bits(masked)))
    }))
}

pub fn assert_matches(gss: &WeightedGss<u8, Bits>, model: &Model, context: &str) {
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
    let expected_top = if model.contains_key(&Vec::new()) {
        None
    } else {
        let mut tops = model.keys().filter_map(|stack| stack.last().copied());
        let first = tops.next();
        if first.is_some() && tops.all(|top| Some(top) == first) {
            first
        } else {
            None
        }
    };
    assert_eq!(gss.top(), expected_top, "top mismatch: {context}");
    assert_eq!(
        gss.has_empty_stack(),
        model.contains_key(&Vec::new()),
        "empty-stack mismatch: {context}"
    );
    assert_eq!(
        gss.joined_weight(),
        model.values().copied().reduce(|a, b| a.join(&b)),
        "joined-weight mismatch: {context}"
    );

    let expected_top_first = canonical(
        model
            .iter()
            .map(|(stack, weight)| (stack.iter().rev().copied().collect(), *weight)),
    );
    let mut visited = Vec::new();
    for_each_stack_top_first(gss, model.len(), |stack, weight| {
        visited.push((stack.to_vec(), *weight));
    })
    .expect("exact visitor bound");
    assert_eq!(
        canonical(visited),
        expected_top_first,
        "visitor mismatch: {context}"
    );

    if !model.is_empty() {
        let mut callbacks = 0usize;
        assert!(
            for_each_stack_top_first(gss, model.len() - 1, |_, _| callbacks += 1).is_err(),
            "visitor accepted too-small bound: {context}"
        );
        assert_eq!(
            callbacks, 0,
            "visitor was not atomic on overflow: {context}"
        );
        assert!(
            gss.to_stacks(model.len() - 1).is_err(),
            "materializer accepted too-small bound: {context}"
        );
    } else {
        assert!(gss.to_stacks(0).unwrap().is_empty());
    }

    if let Some(prefix) = linear_prefix(gss) {
        assert_eq!(
            materialize(&prefix.clone().into_gss()),
            *model,
            "linear-prefix round trip: {context}"
        );

        let visible: Vec<u8> = (0..prefix.len())
            .map(|depth| *prefix.get(depth).expect("depth below prefix length"))
            .collect();
        for stack in model.keys() {
            assert!(
                stack
                    .iter()
                    .rev()
                    .copied()
                    .zip(&visible)
                    .all(|(a, b)| a == *b),
                "linear prefix is not common: {context}"
            );
        }

        if prefix.floor_is_empty() {
            assert_eq!(model.len(), 1, "empty floor with several stacks: {context}");
            assert_eq!(visible.len(), model.keys().next().unwrap().len());
        }

        let pushed_value = 251;
        let mut pushed = prefix.clone();
        pushed.push(pushed_value);
        assert_eq!(
            materialize(&pushed.into_gss()),
            push(model, pushed_value),
            "linear-prefix push mismatch: {context}"
        );

        for count in [0, 1, prefix.len(), prefix.len().saturating_add(2)] {
            let mut popped = prefix.clone();
            let remainder = popped.popn(count);
            let applied = count - remainder;
            assert_eq!(
                materialize(&popped.into_gss()),
                popn(model, applied),
                "linear-prefix pop mismatch: {context}, count={count}, remainder={remainder}"
            );
        }
    }
}
