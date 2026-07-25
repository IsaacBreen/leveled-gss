use crate::Weight;
use crate::effects::StackEffect;
use crate::gss::WeightedGss;
use crate::nodes::{UKind, URef, WKind, u_has_empty, u_segment, w_shared};
use crate::segment::Segment;
use std::collections::VecDeque;
use std::hash::Hash;
use std::sync::Arc;

/// Mutable fast-path view of a linear top prefix over an arbitrary hidden floor.
///
/// A virtual stack is an optimisation probe, not proof that the entire GSS
/// denotes one concrete stack. [`Self::is_complete`] reports whether the hidden
/// floor is exactly the empty stack.
#[derive(Clone)]
pub struct VirtualStack<S, W> {
    segments: VecDeque<Segment<S>>,
    floor: URef<S>,
    weight: Arc<W>,
    pending: Vec<S>,
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
        let mut segments = VecDeque::new();
        let mut cursor = stacks.clone();
        while let UKind::Segment { values, next } = &cursor.kind {
            segments.push_back(values.clone());
            cursor = next.clone();
        }
        if segments.is_empty() {
            return None;
        }
        Some(Self {
            segments,
            floor: cursor,
            weight: weight.clone(),
            pending: Vec::new(),
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
        for segment in &self.segments {
            if depth < segment.len() {
                return segment.get(depth);
            }
            depth -= segment.len();
        }
        None
    }

    /// Number of values available in the visible linear prefix.
    #[must_use]
    pub fn prefix_len(&self) -> usize {
        self.pending.len() + self.segments.iter().map(Segment::len).sum::<usize>()
    }

    /// Return whether the hidden floor is exactly the empty stack.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.floor.paths == 1 && u_has_empty(&self.floor)
    }

    /// Return the shared weight of all alternatives under this prefix.
    #[must_use]
    pub fn weight(&self) -> &W {
        self.weight.as_ref()
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
        if self.prefix_len() == 0 {
            return false;
        }
        let remaining = self.pop_prefix(1);
        debug_assert_eq!(remaining, 0);
        self.push(value);
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
            let Some(front) = self.segments.pop_front() else {
                break;
            };
            if count < front.len() {
                self.segments
                    .push_front(front.drop_front(count).expect("partial segment pop"));
                count = 0;
            } else {
                count -= front.len();
            }
        }
        count
    }

    /// Convert the fast-path view back into a general weighted GSS.
    #[must_use]
    pub fn into_gss(self) -> WeightedGss<S, W> {
        let mut stacks = self.floor;
        for segment in self.segments.into_iter().rev() {
            stacks = u_segment(segment, stacks);
        }
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

    /// Apply nondeterministic effects while sharing the unchanged hidden floor.
    #[must_use]
    pub fn apply_effects<I, P>(self, effects: I) -> WeightedGss<S, W>
    where
        I: IntoIterator<Item = StackEffect<P>>,
        P: AsRef<[S]>,
    {
        let effects: Vec<_> = effects.into_iter().collect();
        if effects
            .iter()
            .any(|effect| effect.pop_count() > self.prefix_len())
        {
            return self.into_gss().apply_effects(effects);
        }
        WeightedGss::merge_all(effects.into_iter().map(|effect| {
            let mut branch = self.clone();
            let remaining = branch.pop_prefix(effect.pop_count());
            debug_assert_eq!(remaining, 0);
            for value in effect.pushed().as_ref() {
                branch.push(value.clone());
            }
            branch.into_gss()
        }))
    }
}
