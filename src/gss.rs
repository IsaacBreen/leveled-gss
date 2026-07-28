use crate::Weight;
use crate::materialize::materialize_stacks;
use crate::nodes::*;
use crate::segment::Segment;
use crate::stack_visit::{StackLimitExceeded, collect_stacks_top_first};
use crate::weight_regions;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;
use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

enum StackTrieChildren<S> {
    Small(SmallVec<[(S, usize); 2]>),
    Large(FxHashMap<S, usize>),
}

impl<S> Default for StackTrieChildren<S> {
    fn default() -> Self {
        Self::Small(SmallVec::new())
    }
}

impl<S> StackTrieChildren<S>
where
    S: Eq + Hash,
{
    fn get(&self, symbol: &S) -> Option<usize> {
        match self {
            Self::Small(children) => children
                .iter()
                .find_map(|(candidate, index)| (candidate == symbol).then_some(*index)),
            Self::Large(children) => children.get(symbol).copied(),
        }
    }

    fn insert(&mut self, symbol: S, index: usize) {
        match self {
            Self::Small(children) if children.len() < 8 => children.push((symbol, index)),
            Self::Small(children) => {
                let mut large = FxHashMap::default();
                large.extend(children.drain(..));
                large.insert(symbol, index);
                *self = Self::Large(large);
            }
            Self::Large(children) => {
                children.insert(symbol, index);
            }
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::Small(children) => children.len(),
            Self::Large(children) => children.len(),
        }
    }
}

struct StackTrieNode<S> {
    parent: Option<usize>,
    symbol: Option<S>,
    children: StackTrieChildren<S>,
    terminal: bool,
}

fn shared_stack_refs<S>(stacks: Vec<Vec<S>>) -> Vec<URef<S>>
where
    S: Clone + Eq + Hash,
{
    let mut nodes = vec![StackTrieNode {
        parent: None,
        symbol: None,
        children: StackTrieChildren::default(),
        terminal: false,
    }];
    let mut terminals = Vec::with_capacity(stacks.len());

    for stack in stacks {
        let mut current = 0usize;
        for symbol in stack {
            let next = if let Some(existing) = nodes[current].children.get(&symbol) {
                existing
            } else {
                let next = nodes.len();
                nodes.push(StackTrieNode {
                    parent: Some(current),
                    symbol: Some(symbol.clone()),
                    children: StackTrieChildren::default(),
                    terminal: false,
                });
                nodes[current].children.insert(symbol, next);
                next
            };
            current = next;
        }
        nodes[current].terminal = true;
        terminals.push(current);
    }

    let significant: Vec<bool> = nodes
        .iter()
        .enumerate()
        .map(|(index, node)| index == 0 || node.terminal || node.children.len() != 1)
        .collect();
    let mut refs: Vec<Option<URef<S>>> = (0..nodes.len()).map(|_| None).collect();
    refs[0] = Some(u_end());

    for index in 1..nodes.len() {
        if !significant[index] {
            continue;
        }

        let mut values = Vec::new();
        let mut current = index;
        loop {
            values.push(
                nodes[current]
                    .symbol
                    .as_ref()
                    .expect("non-root trie nodes have a symbol")
                    .clone(),
            );
            let parent = nodes[current]
                .parent
                .expect("non-root trie nodes have a parent");
            if significant[parent] {
                let floor = refs[parent]
                    .as_ref()
                    .expect("significant ancestors are constructed first")
                    .clone();
                refs[index] = Some(u_segment(Segment::from_top_first(values), floor));
                break;
            }
            current = parent;
        }
    }

    terminals
        .into_iter()
        .map(|terminal| {
            refs[terminal]
                .as_ref()
                .expect("terminal trie nodes are significant")
                .clone()
        })
        .collect()
}

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
    #[must_use]
    pub fn from_stacks_with_weight<I, T>(stacks: I, weight: W) -> Self
    where
        I: IntoIterator<Item = T>,
        T: IntoIterator<Item = S>,
    {
        let concrete: Vec<Vec<S>> = stacks
            .into_iter()
            .map(|stack| stack.into_iter().collect())
            .collect();
        let stacks = u_merge_all(shared_stack_refs(concrete));
        Self {
            root: w_shared(Arc::new(weight), stacks),
        }
    }

    /// Construct from bottom-to-top stack and weight pairs.
    #[must_use]
    pub fn from_stacks<I, T>(entries: I) -> Self
    where
        I: IntoIterator<Item = (T, W)>,
        T: IntoIterator<Item = S>,
    {
        let (stacks, weights): (Vec<Vec<S>>, Vec<W>) = entries
            .into_iter()
            .map(|(stack, weight)| (stack.into_iter().collect(), weight))
            .unzip();
        let stack_refs = shared_stack_refs(stacks);
        Self {
            root: w_merge_all(
                stack_refs
                    .into_iter()
                    .zip(weights)
                    .map(|(stack, weight)| w_shared(Arc::new(weight), stack)),
            ),
        }
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

    #[cfg(feature = "python")]
    pub(crate) fn merge_all(values: impl IntoIterator<Item = Self>) -> Self {
        Self {
            root: w_merge_all(values.into_iter().map(|value| value.root)),
        }
    }

    /// Push `symbol` onto every represented stack.
    #[must_use]
    pub fn push(&self, symbol: S) -> Self {
        Self {
            root: w_push(&self.root, symbol),
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
    /// `max_stacks` bounds the number of distinct concrete stacks returned.
    /// The method returns [`StackLimitExceeded`] instead of silently truncating.
    pub fn to_stacks(&self, max_stacks: usize) -> Result<Vec<(Vec<S>, W)>, StackLimitExceeded> {
        if self.root.paths == 1 {
            let mut stacks = collect_stacks_top_first(self, max_stacks)?;
            for (stack, _) in &mut stacks {
                stack.reverse();
            }
            Ok(stacks)
        } else {
            materialize_stacks(&self.root, max_stacks)
        }
    }
}

impl<S, W> fmt::Debug for WeightedGss<S, W> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WeightedGss")
            .field("is_empty", &self.is_empty())
            .field("max_depth", &self.root.max_depth)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Bits(u8);

    impl Weight for Bits {
        fn join(&self, other: &Self) -> Self {
            Self(self.0 | other.0)
        }
    }

    #[test]
    fn batched_construction_shares_common_stack_floors() {
        let refs = shared_stack_refs(vec![vec![0_u8, 1, 2], vec![0, 1, 3]]);
        let first_floor = match &refs[0].kind {
            UKind::Segment { next, .. } => next,
            UKind::Branch { .. } => panic!("non-empty stack should start with a segment"),
        };
        let second_floor = match &refs[1].kind {
            UKind::Segment { next, .. } => next,
            UKind::Branch { .. } => panic!("non-empty stack should start with a segment"),
        };
        assert!(Arc::ptr_eq(first_floor, second_floor));
    }

    #[test]
    fn shared_floors_make_join_heavy_pop_collapse_immediate() {
        let gss = WeightedGss::from_stacks([
            (vec![0_u8, 1, 2], Bits(1)),
            (vec![0_u8, 1, 3], Bits(2)),
            (vec![0_u8, 1, 4], Bits(4)),
        ]);
        let popped = gss.pop();
        assert_eq!(popped.to_stacks(1).unwrap(), vec![(vec![0, 1], Bits(7))]);
        assert_eq!(popped.root.paths, 1);
    }

    #[test]
    fn deep_weighted_common_top_prefix_is_compact_and_iterative() {
        let depth = 20_000usize;
        let mut left = Vec::with_capacity(depth);
        let mut right = Vec::with_capacity(depth);
        left.push(0_u32);
        right.push(1_u32);
        for value in 1..depth as u32 {
            left.push(value);
            right.push(value);
        }

        let gss = WeightedGss::from_stacks([(left, Bits(1)), (right, Bits(2))]);
        assert_eq!(gss.max_depth(), depth);
        assert_eq!(gss.top(), Some((depth - 1) as u32));
        assert!(matches!(gss.root.kind, WKind::Segment { .. }));

        let popped = gss.popn(depth - 1);
        let mut stacks = popped.to_stacks(2).unwrap();
        stacks.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(stacks, vec![(vec![0], Bits(1)), (vec![1], Bits(2))]);
    }

    #[test]
    fn separately_built_deep_weighted_prefixes_merge_iteratively() {
        let depth = 20_000usize;
        let make = |bottom: u32, weight: Bits| {
            let mut stack = Vec::with_capacity(depth);
            stack.push(bottom);
            stack.extend(1..depth as u32);
            WeightedGss::from_stacks([
                (stack.clone(), weight),
                (
                    {
                        stack[0] = bottom + 10;
                        stack
                    },
                    Bits(weight.0 << 1),
                ),
            ])
        };

        let merged = make(0, Bits(1)).merge(&make(100, Bits(4)));
        assert_eq!(merged.max_depth(), depth);
        assert_eq!(merged.top(), Some((depth - 1) as u32));
        assert_eq!(merged.popn(depth - 1).to_stacks(4).unwrap().len(), 4);
    }

    #[test]
    fn shared_stack_refs_handle_empty_duplicate_and_prefix_stacks() {
        let gss = WeightedGss::from_stacks([
            (Vec::<u8>::new(), Bits(1)),
            (vec![0], Bits(2)),
            (vec![0, 1], Bits(4)),
            (vec![0, 1], Bits(8)),
        ]);
        let mut stacks = gss.to_stacks(3).unwrap();
        stacks.sort_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(
            stacks,
            vec![
                (Vec::new(), Bits(1)),
                (vec![0], Bits(2)),
                (vec![0, 1], Bits(12)),
            ]
        );
    }
}
