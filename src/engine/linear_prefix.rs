use crate::Weight;
use crate::gss::WeightedGss;
use crate::nodes::{UKind, URef, WKind, u_has_empty, u_segment, w_shared};
use crate::segment::Segment;
use smallvec::SmallVec;
use std::hash::Hash;
use std::sync::Arc;

/// Mutable view of a linear top prefix over an unchanged hidden floor.
///
/// The hidden floor may still be branched. Use [`Self::floor_is_empty`] to test
/// whether the prefix represents one complete concrete stack.
#[derive(Clone)]
pub struct LinearPrefix<S, W> {
    values: Option<Segment<S>>,
    next: URef<S>,
    weight: Arc<W>,
    pending: SmallVec<[S; 2]>,
}

impl<S, W> LinearPrefix<S, W>
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

    /// Return the number of values in the accessible linear prefix.
    #[must_use]
    pub fn len(&self) -> usize {
        let current = self.values.as_ref().map_or(0, Segment::len);
        self.pending
            .len()
            .saturating_add(current)
            .saturating_add(segment_chain_len(&self.next))
    }

    /// Return whether the accessible prefix is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Return a visible value by depth from the top.
    #[must_use]
    pub fn get(&self, mut depth_from_top: usize) -> Option<&S> {
        if depth_from_top < self.pending.len() {
            return self.pending.iter().rev().nth(depth_from_top);
        }
        depth_from_top -= self.pending.len();

        if let Some(values) = &self.values {
            if depth_from_top < values.len() {
                return values.get(depth_from_top);
            }
            depth_from_top -= values.len();
        }

        let mut next = &self.next;
        loop {
            match &next.kind {
                UKind::Segment {
                    values,
                    next: following,
                } => {
                    if depth_from_top < values.len() {
                        return values.get(depth_from_top);
                    }
                    depth_from_top -= values.len();
                    next = following;
                }
                UKind::Branch { .. } => return None,
            }
        }
    }

    /// Return whether the hidden floor is exactly the empty stack.
    #[must_use]
    pub fn floor_is_empty(&self) -> bool {
        let floor = segment_chain_floor(&self.next);
        floor.paths == 1 && u_has_empty(floor)
    }

    /// Push one value onto the prefix.
    pub fn push(&mut self, value: S) {
        self.pending.push(value);
    }

    /// Pop values from the prefix and return the number that reached its floor.
    pub fn popn(&mut self, mut count: usize) -> usize {
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
                return 0;
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

    /// Convert the view back into a weighted GSS.
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
