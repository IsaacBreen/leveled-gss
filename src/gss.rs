use crate::Weight;
use crate::nodes::*;
use crate::paths::collect_raw_paths;
use crate::segment::Segment;
use crate::weight_regions;
use rustc_hash::FxHashMap;
use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

/// An unweighted graph-structured stack.
pub type Gss<S> = WeightedGss<S, ()>;

/// A persistent collection of weighted stack alternatives.
///
/// Stacks are supplied and returned bottom-to-top. If several represented
/// alternatives denote the same concrete stack, their weights are joined.
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
    /// Construct a GSS containing no stack alternatives.
    #[must_use]
    pub fn new() -> Self {
        Self { root: w_empty() }
    }

    /// Return whether this GSS contains no alternatives.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        w_is_empty(&self.root)
    }

    /// Return the maximum represented stack depth.
    #[must_use]
    pub fn max_depth(&self) -> usize {
        self.root.max_depth
    }

    #[cfg(feature = "python")]
    pub(crate) fn path_count_at_most(&self, limit: usize) -> usize {
        self.root.paths.min(limit)
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

    /// Construct several bottom-to-top stacks carrying one shared weight.
    ///
    /// This constructor can retain more sharing than repeatedly constructing
    /// and merging the stacks independently.
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

    #[cfg(feature = "python")]
    pub(crate) fn with_stack(&self, stack: impl IntoIterator<Item = S>, weight: W) -> Self {
        self.merge(&Self::from_stack(stack, weight))
    }

    /// Return the union of the alternatives in `self` and `other`.
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        Self {
            root: w_merge(&self.root, &other.root),
        }
    }

    pub(crate) fn merge_all(values: impl IntoIterator<Item = Self>) -> Self {
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
    ///
    /// Empty alternatives underflow and disappear.
    #[must_use]
    pub fn pop(&self) -> Self {
        Self {
            root: w_pop(&self.root),
        }
    }

    /// Pop `count` values, discarding alternatives that underflow.
    #[must_use]
    pub fn popn(&self, count: usize) -> Self {
        Self {
            root: w_popn(&self.root, count),
        }
    }

    /// Return the unique non-empty top value.
    ///
    /// Returns `None` if there is no such value, if several top values are
    /// possible, or if an empty-stack alternative is also present.
    #[must_use]
    pub fn top(&self) -> Option<S> {
        w_single_exclusive_top(&self.root)
    }

    /// Iterate over the distinct non-empty top values in unspecified order.
    pub fn tops(&self) -> impl Iterator<Item = S> {
        w_tops(&self.root).into_iter()
    }

    /// Return whether the empty stack is represented.
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

    /// Retain only the empty-stack alternative.
    #[must_use]
    pub fn retain_empty(&self) -> Self {
        Self {
            root: w_retain_empty(&self.root),
        }
    }

    /// Retain alternatives whose top equals `top`, then pop it.
    #[must_use]
    pub fn pop_top(&self, top: &S) -> Self {
        Self {
            root: w_pop_top(&self.root, top),
        }
    }

    #[cfg(feature = "python")]
    pub(crate) fn pop_branches(&self) -> Vec<(S, Self)> {
        self.tops()
            .map(|top| {
                let remainder = self.pop_top(&top);
                (top, remainder)
            })
            .collect()
    }

    /// Iterate over the weights stored in the factored representation.
    ///
    /// This is not one item per concrete stack. One stored weight may apply to
    /// many stacks, equal values may be yielded more than once, and several
    /// structural paths spelling the same stack may share one yielded weight.
    /// Iteration order and item count are not semantic properties of the GSS.
    /// Shared weighted nodes are visited once.
    pub fn weights(&self) -> impl Iterator<Item = &W> {
        weight_regions::iter(self)
    }

    /// Transform each stored weight region while preserving stack sharing.
    ///
    /// The callback is applied to the factored representation, not once to the
    /// joined weight of each concrete stack. No algebraic law is required. If
    /// the result must be independent of equivalent refactorings, `transform`
    /// should preserve joins:
    ///
    /// ```text
    /// transform(a.join(b)) == transform(a).join(transform(b))
    /// ```
    #[must_use]
    pub fn map_weights<V>(&self, mut transform: impl FnMut(&W) -> V) -> WeightedGss<S, V>
    where
        V: Weight,
    {
        self.filter_map_weights(|weight| Some(transform(weight)))
    }

    /// Transform or discard each stored weight region while preserving sharing.
    ///
    /// The callback is applied to the factored representation, not once to the
    /// joined weight of each concrete stack. `None` removes the complete stack
    /// sublanguage covered by that stored weight. No algebraic law is required.
    ///
    /// For a result independent of equivalent refactorings, lift `join` to
    /// `Option<V>` by treating `None` as no contribution and joining two
    /// `Some` values, then require:
    ///
    /// ```text
    /// transform(a.join(b)) == join_options(transform(a), transform(b))
    /// ```
    #[must_use]
    pub fn filter_map_weights<V>(&self, transform: impl FnMut(&W) -> Option<V>) -> WeightedGss<S, V>
    where
        V: Weight,
    {
        weight_regions::filter_map(self, transform)
    }

    /// Join the weights of all represented alternatives.
    ///
    /// Returns `None` only when this GSS contains no alternatives.
    #[must_use]
    pub fn joined_weight(&self) -> Option<W> {
        w_joined_weight(&self.root)
    }

    #[cfg(feature = "python")]
    pub(crate) fn empty_weight(&self) -> Option<W> {
        w_empty_weight(&self.root)
    }

    /// Materialise canonical bottom-to-top stacks and their joined weights.
    ///
    /// `max_paths` bounds the number of internal structural paths traversed,
    /// rather than the number of distinct output stacks. The method returns an
    /// error instead of silently truncating.
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
}

impl<S, W> fmt::Debug for WeightedGss<S, W> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WeightedGss")
            .field("alternatives", &self.root.paths)
            .field("max_depth", &self.root.max_depth)
            .finish()
    }
}

/// Error returned when bounded materialisation would traverse too many paths.
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
