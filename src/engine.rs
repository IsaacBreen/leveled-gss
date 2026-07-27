//! Optional primitives for high-performance parser and state-machine engines.
//!
//! Enable the `engine` Cargo feature to use this module. Its operations preserve
//! graph sharing and avoid materialising all represented stacks. The ordinary
//! crate API remains limited to weighted-stack semantics.

mod language;
mod linear_prefix;

pub use language::{StackLanguageId, StackLanguageInterner};
pub use linear_prefix::LinearPrefix;

use crate::Weight;
use crate::gss::WeightedGss;
use crate::nodes::*;
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;
use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

/// Visit distinct concrete stacks in top-first order without unbounded expansion.
///
/// At most `limit` callbacks are made. Equal concrete stacks are coalesced and
/// their weights joined before the callback. The common one-path case is handled
/// directly with inline storage; larger shared graphs use a memoised bounded
/// collector whose work is proportional to the represented language up to the
/// chosen limit.
pub fn for_each_stack_top_first<S, W>(
    gss: &WeightedGss<S, W>,
    limit: usize,
    mut visit: impl FnMut(&[S], &W),
) -> Result<(), StackLimitExceeded>
where
    S: Clone + Eq + Hash,
    W: Weight,
{
    if gss.root.paths == 1 {
        let mut stack = SmallVec::<[S; 16]>::new();
        if let Some(weight) = single_weighted_path(&gss.root, &mut stack) {
            if limit == 0 {
                return Err(StackLimitExceeded { limit });
            }
            visit(&stack, weight);
            return Ok(());
        }
    }

    let mut weighted_memo = FxHashMap::default();
    let mut unweighted_memo = FxHashMap::default();
    let stacks =
        collect_weighted_stacks(&gss.root, limit, &mut weighted_memo, &mut unweighted_memo)
            .ok_or(StackLimitExceeded { limit })?;
    for (stack, weight) in stacks.iter() {
        visit(stack, weight);
    }
    Ok(())
}

/// Try to expose a mutable linear top prefix over an unchanged hidden floor.
///
/// Returns `None` when the current representation does not have one homogeneous
/// weight and a directly accessible linear prefix.
#[must_use]
pub fn linear_prefix<S, W>(gss: &WeightedGss<S, W>) -> Option<LinearPrefix<S, W>>
where
    S: Clone + Eq + Hash,
    W: Weight,
{
    LinearPrefix::from_gss(gss)
}

/// Error returned when bounded stack visitation would exceed its stack limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StackLimitExceeded {
    /// Maximum number of distinct concrete stacks allowed by the caller.
    pub limit: usize,
}

impl fmt::Display for StackLimitExceeded {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the weighted GSS contains more than {} distinct stacks",
            self.limit
        )
    }
}

impl std::error::Error for StackLimitExceeded {}

fn single_weighted_path<'a, S, W>(
    node: &'a WRef<S, W>,
    output: &mut SmallVec<[S; 16]>,
) -> Option<&'a W>
where
    S: Clone,
{
    match &node.kind {
        WKind::Shared { weight, stacks } => {
            single_unweighted_path(stacks, output)?;
            Some(weight.as_ref())
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
            single_weighted_path(child, output)
        }
    }
}

fn single_unweighted_path<S>(node: &URef<S>, output: &mut SmallVec<[S; 16]>) -> Option<()>
where
    S: Clone,
{
    match &node.kind {
        UKind::Segment { values, next } => {
            output.extend(values.iter().cloned());
            single_unweighted_path(next, output)
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
            single_unweighted_path(child, output)
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
