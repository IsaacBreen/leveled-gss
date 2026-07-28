use crate::Weight;
use crate::nodes::{UKind, URef, WKind, WRef};
use crate::stack_visit::StackLimitExceeded;
use rustc_hash::FxHashMap;
use std::collections::hash_map::Entry;
use std::hash::Hash;

pub(crate) fn materialize_stacks<S, W>(
    root: &WRef<S, W>,
    max_stacks: usize,
) -> Result<Vec<(Vec<S>, W)>, StackLimitExceeded>
where
    S: Clone + Eq + Hash,
    W: Weight,
{
    let mut stacks = FxHashMap::<Vec<S>, W>::default();
    let mut prefix = Vec::new();
    let complete = walk_weighted(root, &mut prefix, &mut |top_first, weight| {
        let mut stack = top_first.to_vec();
        stack.reverse();
        let at_limit = stacks.len() == max_stacks;
        match stacks.entry(stack) {
            Entry::Occupied(mut entry) => {
                let joined = entry.get().join(weight);
                entry.insert(joined);
                true
            }
            Entry::Vacant(entry) => {
                if at_limit {
                    false
                } else {
                    entry.insert(weight.clone());
                    true
                }
            }
        }
    });
    if complete {
        Ok(stacks.into_iter().collect())
    } else {
        Err(StackLimitExceeded::new())
    }
}

fn walk_weighted<S, W>(
    node: &WRef<S, W>,
    prefix: &mut Vec<S>,
    emit: &mut impl FnMut(&[S], &W) -> bool,
) -> bool
where
    S: Clone,
{
    match &node.kind {
        WKind::Shared { weight, stacks } => {
            walk_unweighted(stacks, prefix, &mut |path| emit(path, weight))
        }
        WKind::Segment { values, next } => {
            let old_len = prefix.len();
            prefix.extend(values.iter().cloned());
            let complete = walk_weighted(next, prefix, emit);
            prefix.truncate(old_len);
            complete
        }
        WKind::Branch { empty, children } => {
            for weight in empty {
                if !emit(prefix, weight) {
                    return false;
                }
            }
            for (top, alternatives) in children {
                prefix.push(top.clone());
                for child in alternatives {
                    if !walk_weighted(child, prefix, emit) {
                        prefix.pop();
                        return false;
                    }
                }
                prefix.pop();
            }
            true
        }
    }
}

fn walk_unweighted<S>(
    node: &URef<S>,
    prefix: &mut Vec<S>,
    emit: &mut impl FnMut(&[S]) -> bool,
) -> bool
where
    S: Clone,
{
    match &node.kind {
        UKind::Branch { empty, children } => {
            if *empty && !emit(prefix) {
                return false;
            }
            for (top, alternatives) in children {
                prefix.push(top.clone());
                for child in alternatives {
                    if !walk_unweighted(child, prefix, emit) {
                        prefix.pop();
                        return false;
                    }
                }
                prefix.pop();
            }
            true
        }
        UKind::Segment { values, next } => {
            let old_len = prefix.len();
            prefix.extend(values.iter().cloned());
            let complete = walk_unweighted(next, prefix, emit);
            prefix.truncate(old_len);
            complete
        }
    }
}
