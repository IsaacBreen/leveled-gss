use crate::Weight;
use crate::gss::WeightedGss;
use crate::nodes::*;
use smallvec::SmallVec;
use std::fmt;
use std::hash::Hash;

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
    if gss.root.paths == 1 {
        let mut stack = SmallVec::<[S; 16]>::new();
        if let Some(weight) = single_weighted_path(&gss.root, &mut stack) {
            if max_stacks == 0 {
                return Err(StackLimitExceeded::new());
            }
            visit(&stack, weight);
            return Ok(());
        }
    }

    let mut stacks = crate::materialize::materialize_stacks(&gss.root, max_stacks)?;
    for (stack, weight) in &mut stacks {
        stack.reverse();
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

    let mut stacks = crate::materialize::materialize_stacks(&gss.root, max_stacks)?;
    for (stack, _) in &mut stacks {
        stack.reverse();
    }
    Ok(stacks)
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
            WKind::Segment { values, next } => {
                output.extend(values.iter().cloned());
                node = next;
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
