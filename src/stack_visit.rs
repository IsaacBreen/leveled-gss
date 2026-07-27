use crate::Weight;
use crate::gss::WeightedGss;
use crate::nodes::*;
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;
use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

/// Error returned when a bounded operation would materialise too many stacks.
///
/// The error is intentionally opaque: the caller already supplied the limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StackLimitExceeded(());

impl StackLimitExceeded {
    pub(crate) const fn new() -> Self {
        Self(())
    }
}

impl fmt::Display for StackLimitExceeded {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the weighted GSS exceeds the configured distinct-stack limit")
    }
}

impl std::error::Error for StackLimitExceeded {}

/// Visit distinct concrete stacks in top-first order without unbounded expansion.
///
/// At most `max_stacks` callbacks are made. Equal concrete stacks are coalesced
/// and their weights joined before the callback. Returns [`StackLimitExceeded`]
/// without invoking the callback when the complete result would exceed the
/// supplied limit.
pub fn for_each_stack_top_first<S, W>(
    gss: &WeightedGss<S, W>,
    max_stacks: usize,
    mut visit: impl FnMut(&[S], &W),
) -> Result<(), StackLimitExceeded>
where
    S: Clone + Eq + Hash,
    W: Weight,
{
    let stacks = collect_stacks_top_first(gss, max_stacks)?;
    for (stack, weight) in &stacks {
        visit(stack, weight);
    }
    Ok(())
}

pub(crate) fn collect_stacks_top_first<S, W>(
    gss: &WeightedGss<S, W>,
    max_stacks: usize,
) -> Result<Vec<(Vec<S>, W)>, StackLimitExceeded>
where
    S: Clone + Eq + Hash,
    W: Weight,
{
    if gss.root.paths == 1 {
        let mut stack = SmallVec::<[S; 16]>::new();
        if let Some(weight) = single_weighted_path(&gss.root, &mut stack) {
            if max_stacks == 0 {
                return Err(StackLimitExceeded::new());
            }
            return Ok(vec![(stack.into_vec(), weight.clone())]);
        }
    }

    let mut weighted_memo = FxHashMap::default();
    let mut unweighted_memo = FxHashMap::default();
    let stacks = collect_weighted_stacks(
        &gss.root,
        max_stacks,
        &mut weighted_memo,
        &mut unweighted_memo,
    )
    .ok_or_else(StackLimitExceeded::new)?;
    Ok(Arc::unwrap_or_clone(stacks))
}

fn single_weighted_path<'a, S, W>(
    mut node: &'a WRef<S, W>,
    output: &mut SmallVec<[S; 16]>,
) -> Option<&'a W>
where
    S: Clone,
{
    loop {
        match &node.kind {
            WKind::Shared { weight, stacks } => {
                single_unweighted_path(stacks, output)?;
                return Some(weight.as_ref());
            }
            WKind::Branch { empty, children } => {
                if empty.len() == 1 && children.is_empty() {
                    return Some(empty[0].as_ref());
                }
                let mut entries = children
                    .iter()
                    .flat_map(|(top, values)| values.iter().map(move |child| (top, child)));
                let (top, child) = entries.next()?;
                if entries.next().is_some() || !empty.is_empty() {
                    return None;
                }
                output.push(top.clone());
                node = child;
            }
        }
    }
}

fn single_unweighted_path<S>(mut node: &URef<S>, output: &mut SmallVec<[S; 16]>) -> Option<()>
where
    S: Clone,
{
    loop {
        match &node.kind {
            UKind::Segment { values, next } => {
                output.extend(values.iter().cloned());
                node = next;
            }
            UKind::Branch { empty, children } => {
                if *empty && children.is_empty() {
                    return Some(());
                }
                let mut entries = children
                    .iter()
                    .flat_map(|(top, values)| values.iter().map(move |child| (top, child)));
                let (top, child) = entries.next()?;
                if entries.next().is_some() || *empty {
                    return None;
                }
                output.push(top.clone());
                node = child;
            }
        }
    }
}

type WeightedStacks<S, W> = Arc<Vec<(Vec<S>, W)>>;
type UnweightedStacks<S> = Arc<Vec<Vec<S>>>;

fn collect_weighted_stacks<S, W>(
    node: &WRef<S, W>,
    limit: usize,
    weighted_memo: &mut FxHashMap<usize, Option<WeightedStacks<S, W>>>,
    unweighted_memo: &mut FxHashMap<usize, Option<UnweightedStacks<S>>>,
) -> Option<WeightedStacks<S, W>>
where
    S: Clone + Eq + Hash,
    W: Weight,
{
    let id = w_id(node);
    if let Some(cached) = weighted_memo.get(&id) {
        return cached.clone();
    }
    let result = match &node.kind {
        WKind::Shared { weight, stacks } => {
            let stacks = collect_unweighted_stacks(stacks, limit, unweighted_memo)?;
            Some(Arc::new(
                stacks
                    .iter()
                    .cloned()
                    .map(|stack| (stack, weight.as_ref().clone()))
                    .collect(),
            ))
        }
        WKind::Branch { empty, children } => {
            let mut stacks = FxHashMap::<Vec<S>, W>::default();
            for weight in empty {
                stacks
                    .entry(Vec::new())
                    .and_modify(|current| *current = current.join(weight.as_ref()))
                    .or_insert_with(|| weight.as_ref().clone());
            }
            if stacks.len() > limit {
                None
            } else {
                let mut complete = true;
                'children: for (top, alternatives) in children {
                    for child in alternatives {
                        let Some(child_stacks) =
                            collect_weighted_stacks(child, limit, weighted_memo, unweighted_memo)
                        else {
                            complete = false;
                            break 'children;
                        };
                        for (suffix, weight) in child_stacks.iter() {
                            let mut stack = Vec::with_capacity(suffix.len() + 1);
                            stack.push(top.clone());
                            stack.extend(suffix.iter().cloned());
                            stacks
                                .entry(stack)
                                .and_modify(|current| *current = current.join(weight))
                                .or_insert_with(|| weight.clone());
                            if stacks.len() > limit {
                                complete = false;
                                break 'children;
                            }
                        }
                    }
                }
                complete.then(|| Arc::new(stacks.into_iter().collect()))
            }
        }
    };
    weighted_memo.insert(id, result.clone());
    result
}

fn collect_unweighted_stacks<S>(
    node: &URef<S>,
    limit: usize,
    memo: &mut FxHashMap<usize, Option<UnweightedStacks<S>>>,
) -> Option<UnweightedStacks<S>>
where
    S: Clone + Eq + Hash,
{
    let id = u_id(node);
    if let Some(cached) = memo.get(&id) {
        return cached.clone();
    }
    let result = match &node.kind {
        UKind::Branch { empty, children } => {
            let mut stacks = FxHashSet::<Vec<S>>::default();
            if *empty {
                stacks.insert(Vec::new());
            }
            if stacks.len() > limit {
                None
            } else {
                let mut complete = true;
                'children: for (top, alternatives) in children {
                    for child in alternatives {
                        let Some(child_stacks) = collect_unweighted_stacks(child, limit, memo)
                        else {
                            complete = false;
                            break 'children;
                        };
                        for suffix in child_stacks.iter() {
                            let mut stack = Vec::with_capacity(suffix.len() + 1);
                            stack.push(top.clone());
                            stack.extend(suffix.iter().cloned());
                            stacks.insert(stack);
                            if stacks.len() > limit {
                                complete = false;
                                break 'children;
                            }
                        }
                    }
                }
                complete.then(|| Arc::new(stacks.into_iter().collect()))
            }
        }
        UKind::Segment { values, next } => {
            let suffixes = collect_unweighted_stacks(next, limit, memo)?;
            Some(Arc::new(
                suffixes
                    .iter()
                    .map(|suffix| {
                        let mut stack = Vec::with_capacity(values.len() + suffix.len());
                        stack.extend(values.iter().cloned());
                        stack.extend(suffix.iter().cloned());
                        stack
                    })
                    .collect(),
            ))
        }
    };
    memo.insert(id, result.clone());
    result
}
