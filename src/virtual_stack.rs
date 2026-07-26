use crate::Weight;
use crate::gss::WeightedGss;
use crate::nodes::{UKind, URef, WKind, u_has_empty, u_segment, w_shared};
use crate::segment::Segment;
use crate::stack_op::StackOp;
use smallvec::SmallVec;
use std::hash::Hash;
use std::sync::Arc;

/// Mutable fast-path view of a linear top prefix over an arbitrary hidden floor.
///
/// A virtual stack is an optimisation probe, not proof that the entire GSS
/// denotes one concrete stack. [`Self::is_complete`] reports whether the hidden
/// floor is exactly the empty stack.
#[derive(Clone)]
pub struct VirtualStack<S, W> {
    values: Option<Segment<S>>,
    next: URef<S>,
    weight: Arc<W>,
    pending: SmallVec<[S; 2]>,
}

impl<S, W> VirtualStack<S, W>
where
    S: Clone + Eq + Hash,
    W: Weight,
{
    pub(crate) fn from_gss(gss: &WeightedGss<S, W>) -> Option<Self> {
        let WKind::Shared { weight, stacks } = &gss.root.kind else {
            return None;
        };
        let UKind::Segment { values, next } = &stacks.kind else {
            return None;
        };
        Some(Self {
            values: Some(values.clone()),
            next: next.clone(),
            weight: weight.clone(),
            pending: SmallVec::new(),
        })
    }

    /// Return the visible top value.
    #[must_use]
    pub fn top(&self) -> Option<&S> {
        self.get_from_top(0)
    }

    /// Return a visible value by depth from the top.
    #[must_use]
    pub fn get_from_top(&self, mut depth: usize) -> Option<&S> {
        if depth < self.pending.len() {
            return self.pending.iter().rev().nth(depth);
        }
        depth -= self.pending.len();

        if let Some(values) = &self.values {
            if depth < values.len() {
                return values.get(depth);
            }
            depth -= values.len();
        }

        let mut next = &self.next;
        loop {
            match &next.kind {
                UKind::Segment {
                    values,
                    next: following,
                } => {
                    if depth < values.len() {
                        return values.get(depth);
                    }
                    depth -= values.len();
                    next = following;
                }
                UKind::Branch { .. } => return None,
            }
        }
    }

    /// Number of values available in the visible linear prefix.
    #[must_use]
    pub fn prefix_len(&self) -> usize {
        let current = self.values.as_ref().map_or(0, Segment::len);
        self.pending
            .len()
            .saturating_add(current)
            .saturating_add(segment_chain_len(&self.next))
    }

    /// Return whether the hidden floor is exactly the empty stack.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        let floor = segment_chain_floor(&self.next);
        floor.paths == 1 && u_has_empty(floor)
    }

    /// Push one value onto the visible prefix.
    pub fn push(&mut self, value: S) {
        self.pending.push(value);
    }

    /// Replace the visible top value, returning false when the prefix is empty.
    pub fn replace_top(&mut self, value: S) -> bool {
        if let Some(top) = self.pending.last_mut() {
            *top = value;
            return true;
        }
        if self.values.is_none() {
            return false;
        }
        let remaining = self.pop_prefix(1);
        debug_assert_eq!(remaining, 0);
        self.pending.push(value);
        true
    }

    /// Pop from the visible prefix and return the unconsumed pop count.
    ///
    /// A nonzero result means the requested pop reached the hidden floor.
    pub fn pop_prefix(&mut self, mut count: usize) -> usize {
        while count > 0 && !self.pending.is_empty() {
            self.pending.pop();
            count -= 1;
        }

        while count > 0 {
            let Some(values) = self.values.take() else {
                break;
            };
            if count < values.len() {
                self.values = values.drop_front(count);
                count = 0;
                break;
            }
            count -= values.len();
            match &self.next.kind {
                UKind::Segment { values, next } => {
                    self.values = Some(values.clone());
                    self.next = next.clone();
                }
                UKind::Branch { .. } => {
                    self.values = None;
                    break;
                }
            }
        }
        count
    }

    /// Convert the fast-path view back into a general weighted GSS.
    #[must_use]
    pub fn into_gss(self) -> WeightedGss<S, W> {
        let mut stacks = match self.values {
            Some(values) => u_segment(values, self.next),
            None => self.next,
        };
        if !self.pending.is_empty() {
            stacks = u_segment(
                Segment::from_top_first(self.pending.into_iter().rev().collect()),
                stacks,
            );
        }
        WeightedGss {
            root: w_shared(self.weight, stacks),
        }
    }

    /// Apply nondeterministic operations while sharing the unchanged hidden floor.
    #[must_use]
    pub fn apply_ops<I, P>(self, ops: I) -> WeightedGss<S, W>
    where
        I: IntoIterator<Item = StackOp<P>>,
        P: AsRef<[S]>,
    {
        let ops: Vec<_> = ops.into_iter().collect();
        if ops.iter().any(|op| op.pop_count() > self.prefix_len()) {
            return self.into_gss().apply_ops(ops);
        }
        WeightedGss::merge_all(ops.into_iter().map(|op| {
            let mut branch = self.clone();
            let remaining = branch.pop_prefix(op.pop_count());
            debug_assert_eq!(remaining, 0);
            for value in op.pushed().as_ref() {
                branch.push(value.clone());
            }
            branch.into_gss()
        }))
    }
}

fn segment_chain_len<S>(mut node: &URef<S>) -> usize {
    let mut len = 0usize;
    while let UKind::Segment { values, next } = &node.kind {
        len = len.saturating_add(values.len());
        node = next;
    }
    len
}

fn segment_chain_floor<S>(mut node: &URef<S>) -> &URef<S> {
    while let UKind::Segment { next, .. } = &node.kind {
        node = next;
    }
    node
}
