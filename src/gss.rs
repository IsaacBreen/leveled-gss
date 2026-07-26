use crate::Weight;
use crate::diagnostics::{RepresentationId, StructuralStats};
use crate::effects::StackEffect;
use crate::nodes::*;
use crate::paths::{Paths, collect_raw_paths};
use crate::segment::Segment;
use crate::virtual_stack::VirtualStack;
use rustc_hash::FxHashMap;
use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

/// An unweighted graph-structured stack.
pub type Gss<S> = WeightedGss<S, ()>;

/// A persistent graph-structured collection of weighted stack alternatives.
///
/// Each structural path spells a stack and carries a weight. Several paths may
/// spell the same concrete stack; the extensional weight of that stack is the
/// join of all corresponding path weights.
pub struct WeightedGss<S, W> {
    pub(crate) root: WRef<S, W>,
}

impl<S, W> Clone for WeightedGss<S, W> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
        }
    }
}

impl<S, W> Default for WeightedGss<S, W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, W> WeightedGss<S, W> {
    /// Construct a GSS containing no alternatives.
    #[must_use]
    pub fn new() -> Self {
        Self { root: w_empty() }
    }

    /// Return whether no alternatives are represented.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        w_is_empty(&self.root)
    }

    /// Return the maximum concrete stack depth.
    #[must_use]
    pub fn max_depth(&self) -> usize {
        self.root.max_depth
    }

    /// Return an opaque process-local representation identity.
    #[must_use]
    pub fn representation_id(&self) -> RepresentationId {
        RepresentationId(w_representation_id(&self.root))
    }

    /// Return representation-level statistics.
    #[must_use]
    pub fn structural_stats(&self) -> StructuralStats {
        crate::paths::structural_stats(&self.root)
    }

    /// Access explicitly path-local operations.
    #[must_use]
    pub fn paths(&self) -> Paths<'_, S, W> {
        Paths::new(self)
    }
}

impl<S, W> WeightedGss<S, W>
where
    S: Clone + Eq + Hash,
    W: Weight,
{
    fn from_stack_with_end(stack: impl IntoIterator<Item = S>, weight: W, end: &URef<S>) -> Self {
        let values: Vec<S> = stack.into_iter().collect();
        let stacks = if values.is_empty() {
            end.clone()
        } else {
            u_segment(
                Segment::from_top_first(values.into_iter().rev().collect()),
                end.clone(),
            )
        };
        Self {
            root: w_shared(Arc::new(weight), stacks),
        }
    }

    /// Construct a GSS containing one bottom-to-top stack.
    #[must_use]
    pub fn from_stack(stack: impl IntoIterator<Item = S>, weight: W) -> Self {
        Self::from_stack_with_end(stack, weight, &u_end())
    }

    /// Construct a GSS from multiple stacks that all carry one shared weight.
    ///
    /// This is the preferred constructor for homogeneous alternatives. It
    /// factors the weight once and builds one shared unweighted stack DAG, so
    /// linear-prefix fast paths remain available above any common branched floor.
    #[must_use]
    pub fn from_stacks_with_weight<I, T>(stacks: I, weight: W) -> Self
    where
        I: IntoIterator<Item = T>,
        T: IntoIterator<Item = S>,
    {
        let end = u_end();
        let stacks = u_merge_all(stacks.into_iter().map(|stack| {
            let values: Vec<S> = stack.into_iter().collect();
            if values.is_empty() {
                end.clone()
            } else {
                u_segment(
                    Segment::from_top_first(values.into_iter().rev().collect()),
                    end.clone(),
                )
            }
        }));
        Self {
            root: w_shared(Arc::new(weight), stacks),
        }
    }

    /// Construct from bottom-to-top stack and weight pairs.
    ///
    /// The representation may retain duplicate concrete stacks as distinct
    /// structural paths. Their extensional weights are joined by observations
    /// such as [`Self::to_stacks`].
    #[must_use]
    pub fn from_stacks<I, T>(stacks: I) -> Self
    where
        I: IntoIterator<Item = (T, W)>,
        T: IntoIterator<Item = S>,
    {
        let end = u_end();
        Self::merge_all(
            stacks
                .into_iter()
                .map(|(stack, weight)| Self::from_stack_with_end(stack, weight, &end)),
        )
    }

    /// Return a new value containing all existing alternatives plus `stack`.
    #[must_use]
    pub fn with_stack(&self, stack: impl IntoIterator<Item = S>, weight: W) -> Self {
        self.merge(&Self::from_stack(stack, weight))
    }

    /// Merge two weighted GSS values.
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        Self {
            root: w_merge(&self.root, &other.root),
        }
    }

    /// Merge an iterable of weighted GSS values using a balanced reduction.
    #[must_use]
    pub fn merge_all(values: impl IntoIterator<Item = Self>) -> Self {
        Self {
            root: w_merge_all(values.into_iter().map(|value| value.root)),
        }
    }

    /// Push `value` onto every represented stack.
    #[must_use]
    pub fn push(&self, value: S) -> Self {
        Self {
            root: w_push(&self.root, value),
        }
    }

    /// Pop one value from every non-empty stack.
    #[must_use]
    pub fn pop(&self) -> Self {
        Self {
            root: w_pop(&self.root),
        }
    }

    /// Pop `count` values from every stack, discarding underflowing alternatives.
    #[must_use]
    pub fn pop_n(&self, count: usize) -> Self {
        Self {
            root: w_pop_n(&self.root, count),
        }
    }

    /// Return the only distinct non-empty top value when no empty alternative exists.
    #[must_use]
    pub fn top(&self) -> Option<S> {
        w_single_exclusive_top(&self.root)
    }

    /// Return each distinct non-empty top value once, in unspecified order.
    #[must_use]
    pub fn tops(&self) -> Tops<S> {
        Tops {
            inner: w_tops(&self.root).into_iter(),
        }
    }

    /// Return whether at least one structural path spells the empty stack.
    #[must_use]
    pub fn has_empty_stack(&self) -> bool {
        w_has_empty(&self.root)
    }

    /// Retain alternatives whose top equals `top`, without popping it.
    #[must_use]
    pub fn retain_top(&self, top: &S) -> Self {
        Self {
            root: w_retain_top(&self.root, top),
        }
    }

    /// Retain only empty-stack alternatives.
    #[must_use]
    pub fn retain_empty(&self) -> Self {
        Self {
            root: w_retain_empty(&self.root),
        }
    }

    /// Retain alternatives whose top equals `top`, then pop that top value.
    #[must_use]
    pub fn pop_top(&self, top: &S) -> Self {
        Self {
            root: w_pop_top(&self.root, top),
        }
    }

    /// Return each distinct top value paired with its popped remainder GSS.
    #[must_use]
    pub fn pop_branches(&self) -> TopBranches<S, W> {
        let branches = self
            .tops()
            .map(|top| TopBranch {
                remainder: self.pop_top(&top),
                top,
            })
            .collect::<Vec<_>>();
        TopBranches {
            inner: branches.into_iter(),
        }
    }

    /// Retain alternatives whose value at `depth_from_top` satisfies `keep`.
    ///
    /// Depth zero is the top. Alternatives too short to contain that position
    /// are discarded. Retained stacks are otherwise unchanged.
    #[must_use]
    pub fn retain_at_depth<F>(&self, depth_from_top: usize, mut keep: F) -> Self
    where
        F: FnMut(&S) -> bool,
    {
        Self {
            root: w_retain_at_depth(&self.root, depth_from_top, &mut keep),
        }
    }

    /// Apply one pop-then-push effect to every represented alternative.
    #[must_use]
    pub fn apply_effect<P>(&self, effect: StackEffect<P>) -> Self
    where
        P: AsRef<[S]>,
    {
        let mut result = self.pop_n(effect.pop_count());
        for value in effect.pushed().as_ref() {
            result = result.push(value.clone());
        }
        result
    }

    /// Nondeterministically apply every effect to every represented alternative.
    #[must_use]
    pub fn apply_effects<I, P>(&self, effects: I) -> Self
    where
        I: IntoIterator<Item = StackEffect<P>>,
        P: AsRef<[S]>,
    {
        Self::merge_all(effects.into_iter().map(|effect| self.apply_effect(effect)))
    }

    /// Apply effects only to alternatives with matching current top values.
    ///
    /// Multiple entries for the same top value represent nondeterministic choices.
    #[must_use]
    pub fn apply_top_effects<I, P>(&self, effects: I) -> Self
    where
        I: IntoIterator<Item = (S, StackEffect<P>)>,
        P: AsRef<[S]>,
    {
        Self::merge_all(
            effects
                .into_iter()
                .map(|(top, effect)| self.retain_top(&top).apply_effect(effect)),
        )
    }

    /// Join all path weights, or return `None` when empty.
    #[must_use]
    pub fn joined_weight(&self) -> Option<W> {
        w_joined_weight(&self.root)
    }

    /// Return the joined weight of all empty-stack paths.
    #[must_use]
    pub fn empty_weight(&self) -> Option<W> {
        w_empty_weight(&self.root)
    }

    /// Materialize extensional bottom-to-top stacks and joined weights.
    ///
    /// `max_paths` bounds structural paths traversed, not the number of distinct
    /// result stacks. The method never silently truncates.
    pub fn to_stacks(&self, max_paths: usize) -> Result<Vec<(Vec<S>, W)>, PathLimitExceeded> {
        let raw = collect_raw_paths(&self.root, max_paths)?;
        let mut canonical: FxHashMap<Vec<S>, W> = FxHashMap::default();
        for (stack, weight) in raw {
            canonical
                .entry(stack)
                .and_modify(|current| *current = current.join(&weight))
                .or_insert(weight);
        }
        Ok(canonical.into_iter().collect())
    }

    /// Try to expose a linear top prefix for caller-controlled fast paths.
    #[must_use]
    pub fn try_virtual_stack(&self) -> Option<VirtualStack<S, W>> {
        VirtualStack::from_gss(self)
    }
}

impl<S, W> FromIterator<(Vec<S>, W)> for WeightedGss<S, W>
where
    S: Clone + Eq + Hash,
    W: Weight,
{
    fn from_iter<T: IntoIterator<Item = (Vec<S>, W)>>(iter: T) -> Self {
        Self::from_stacks(iter)
    }
}

impl<S, W> fmt::Debug for WeightedGss<S, W> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WeightedGss")
            .field("paths", &self.root.paths)
            .field("max_depth", &self.root.max_depth)
            .field("representation", &self.representation_id())
            .finish()
    }
}

/// One popped top-symbol branch.
pub struct TopBranch<S, W> {
    /// The selected top symbol.
    pub top: S,
    /// Alternatives from that branch after popping the selected top.
    pub remainder: WeightedGss<S, W>,
}

/// Iterator over distinct top symbols.
pub struct Tops<S> {
    inner: smallvec::IntoIter<[S; 8]>,
}

impl<S> Iterator for Tops<S> {
    type Item = S;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<S> ExactSizeIterator for Tops<S> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

/// Iterator over popped top-symbol branches.
pub struct TopBranches<S, W> {
    inner: std::vec::IntoIter<TopBranch<S, W>>,
}

impl<S, W> Iterator for TopBranches<S, W> {
    type Item = TopBranch<S, W>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

impl<S, W> ExactSizeIterator for TopBranches<S, W> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

/// Error returned when bounded path expansion would exceed the chosen limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PathLimitExceeded {
    /// Maximum structural path count allowed by the caller.
    pub limit: usize,
}

impl fmt::Display for PathLimitExceeded {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the weighted GSS contains more than {} structural paths",
            self.limit
        )
    }
}

impl std::error::Error for PathLimitExceeded {}
