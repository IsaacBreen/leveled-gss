use crate::Weight;
use crate::diagnostics::StructuralStats;
use crate::gss::{Gss, PathLimitExceeded, WeightedGss};
use crate::nodes::*;
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;
use std::hash::Hash;
use std::sync::Arc;

/// Explicit access to path-local operations and structural paths.
pub struct Paths<'a, S, W> {
    gss: &'a WeightedGss<S, W>,
}

impl<'a, S, W> Paths<'a, S, W> {
    pub(crate) const fn new(gss: &'a WeightedGss<S, W>) -> Self {
        Self { gss }
    }

    /// Count structural paths, capped at `limit`.
    #[must_use]
    pub fn count_at_most(&self, limit: usize) -> usize {
        self.gss.root.paths.min(limit)
    }

    /// Visit each distinct stored path-weight node without expanding stack paths.
    pub fn for_each_weight(&self, mut visit: impl FnMut(&W)) {
        for_each_weight_node(&self.gss.root, &mut visit);
    }

    /// Return whether every distinct stored path-weight node satisfies `predicate`.
    pub fn all_weights_satisfy(&self, mut predicate: impl FnMut(&W) -> bool) -> bool {
        all_weight_nodes_satisfy(&self.gss.root, &mut predicate)
    }
}

impl<'gss, S, W> Paths<'gss, S, W>
where
    S: Clone + Eq + Hash,
    W: Weight,
{
    /// Materialize raw structural paths as bottom-to-top stacks.
    ///
    /// Duplicate concrete stacks may appear with separate weights.
    pub fn to_vec(&self, max_paths: usize) -> Result<Vec<(Vec<S>, W)>, PathLimitExceeded> {
        collect_raw_paths(&self.gss.root, max_paths)
    }

    /// Visit raw structural paths as top-first stack slices without materializing them.
    ///
    /// Duplicate concrete stacks may be visited more than once. At most
    /// `max_paths` callbacks are made; if more paths exist, the method then
    /// returns [`PathLimitExceeded`].
    pub fn for_each_top_first(
        &self,
        max_paths: usize,
        mut visit: impl FnMut(&[S], &W),
    ) -> Result<(), PathLimitExceeded> {
        let mut prefix = SmallVec::<[S; 64]>::new();
        let mut emitted = 0usize;
        if walk_w_bounded(
            &self.gss.root,
            &mut prefix,
            max_paths,
            &mut emitted,
            &mut visit,
        ) {
            Ok(())
        } else {
            Err(PathLimitExceeded { limit: max_paths })
        }
    }

    /// Fill `output` with the only structural path in top-first order.
    ///
    /// Returns its stored weight, or `None` unless exactly one structural path
    /// is represented. The output buffer is cleared before use.
    pub fn single_top_first(&self, output: &mut Vec<S>) -> Option<&'gss W> {
        if self.gss.root.paths != 1 {
            return None;
        }
        output.clear();
        single_w_into(&self.gss.root, output)
    }

    /// Invoke `visit` with the only structural path in top-first order.
    ///
    /// Returns `None` unless exactly one structural path is represented. The
    /// temporary path stays inline for depths up to 16, avoiding a heap
    /// allocation in common linear-stack fast paths.
    pub fn with_single_top_first<R>(&self, visit: impl FnOnce(&[S], &'gss W) -> R) -> Option<R> {
        if self.gss.root.paths != 1 {
            return None;
        }
        let mut output = SmallVec::<[S; 16]>::new();
        let weight = single_w_into(&self.gss.root, &mut output)?;
        Some(visit(&output, weight))
    }

    /// Write the only structural path into an inline top-first buffer.
    ///
    /// This is the allocation-free counterpart of [`Self::single_top_first`]
    /// for hot paths that already use a [`SmallVec`]. The output is cleared
    /// before use.
    pub fn single_top_first_small(&self, output: &mut SmallVec<[S; 16]>) -> Option<&'gss W> {
        if self.gss.root.paths != 1 {
            return None;
        }
        output.clear();
        single_w_into(&self.gss.root, output)
    }

    /// Transform every stored path-local weight while preserving sharing.
    #[must_use]
    pub fn map_weights<V, F>(&self, mut map: F) -> WeightedGss<S, V>
    where
        V: Weight,
        F: FnMut(&W) -> V,
    {
        self.try_map_weights::<V, std::convert::Infallible, _>(|weight| Ok(map(weight)))
            .unwrap_or_else(|never| match never {})
    }

    /// Fallibly transform every stored path-local weight.
    pub fn try_map_weights<V, E, F>(&self, mut map: F) -> Result<WeightedGss<S, V>, E>
    where
        V: Weight,
        F: FnMut(&W) -> Result<V, E>,
    {
        let mut w_memo = FxHashMap::default();
        Ok(WeightedGss {
            root: try_map_w(&self.gss.root, &mut w_memo, &mut |weight| {
                map(weight).map(Some)
            })?,
        })
    }

    /// Transform and optionally discard stored path-local weights.
    #[must_use]
    pub fn filter_map_weights<V, F>(&self, mut map: F) -> WeightedGss<S, V>
    where
        V: Weight,
        F: FnMut(&W) -> Option<V>,
    {
        self.try_filter_map_weights::<V, std::convert::Infallible, _>(|weight| Ok(map(weight)))
            .unwrap_or_else(|never| match never {})
    }

    /// Fallibly transform and optionally discard stored path-local weights.
    pub fn try_filter_map_weights<V, E, F>(&self, mut map: F) -> Result<WeightedGss<S, V>, E>
    where
        V: Weight,
        F: FnMut(&W) -> Result<Option<V>, E>,
    {
        let mut w_memo = FxHashMap::default();
        Ok(WeightedGss {
            root: try_map_w(&self.gss.root, &mut w_memo, &mut map)?,
        })
    }

    /// Partition structural paths by equal stored weight values.
    ///
    /// Each returned unweighted GSS retains exactly the structural paths carrying
    /// the paired weight.
    #[must_use]
    pub fn partition_by_weight(&self) -> Vec<(W, Gss<S>)>
    where
        W: PartialEq,
    {
        let mut weights = Vec::<W>::new();
        collect_distinct_weights(&self.gss.root, &mut FxHashSet::default(), &mut weights);
        weights
            .into_iter()
            .map(|weight| {
                let stacks = select_weight_as_unweighted(&self.gss.root, &weight);
                let paths = WeightedGss {
                    root: w_shared(Arc::new(()), stacks),
                };
                (weight, paths)
            })
            .collect()
    }
}

fn select_weight_as_unweighted<S, W>(node: &WRef<S, W>, target: &W) -> URef<S>
where
    S: Clone + Eq + Hash,
    W: PartialEq,
{
    match &node.kind {
        WKind::Shared { weight, stacks } => {
            if weight.as_ref() == target {
                stacks.clone()
            } else {
                u_empty()
            }
        }
        WKind::Branch { empty, children } => {
            let keep_empty = empty.iter().any(|weight| weight.as_ref() == target);
            let mut kept = UChildren::default();
            for (top, values) in children {
                for child in values {
                    let child = select_weight_as_unweighted(child, target);
                    if !u_is_empty(&child) {
                        kept.entry(top.clone()).or_default().push(child);
                    }
                }
            }
            u_branch(keep_empty, kept)
        }
    }
}

fn try_map_w<S, W, V, E, F>(
    node: &WRef<S, W>,
    w_memo: &mut FxHashMap<usize, WRef<S, V>>,
    map: &mut F,
) -> Result<WRef<S, V>, E>
where
    S: Clone + Eq + Hash,
    W: Weight,
    V: Weight,
    F: FnMut(&W) -> Result<Option<V>, E>,
{
    let id = w_id(node);
    if let Some(cached) = w_memo.get(&id) {
        return Ok(cached.clone());
    }
    let mapped = match &node.kind {
        WKind::Shared { weight, stacks } => match map(weight.as_ref())? {
            // Weight transformations do not alter stack symbols. Keep the
            // immutable unweighted DAG by Arc rather than rebuilding it.
            Some(weight) => w_shared(Arc::new(weight), stacks.clone()),
            None => w_empty(),
        },
        WKind::Branch { empty, children } => {
            let mut mapped_empty = SmallVec::new();
            for weight in empty {
                if let Some(weight) = map(weight.as_ref())? {
                    mapped_empty.push(Arc::new(weight));
                }
            }
            let mut mapped_children = WChildren::default();
            for (top, values) in children {
                for child in values {
                    let child = try_map_w(child, w_memo, map)?;
                    if !w_is_empty(&child) {
                        mapped_children.entry(top.clone()).or_default().push(child);
                    }
                }
            }
            w_branch(mapped_empty, mapped_children)
        }
    };
    w_memo.insert(id, mapped.clone());
    Ok(mapped)
}

fn collect_distinct_weights<S, W>(
    node: &WRef<S, W>,
    seen: &mut FxHashSet<usize>,
    weights: &mut Vec<W>,
) where
    W: Clone + PartialEq,
{
    if !seen.insert(w_id(node)) {
        return;
    }
    let mut add = |weight: &W| {
        if !weights.contains(weight) {
            weights.push(weight.clone());
        }
    };
    match &node.kind {
        WKind::Shared { weight, .. } => add(weight.as_ref()),
        WKind::Branch { empty, children } => {
            for weight in empty {
                add(weight.as_ref());
            }
            for child in children.values().flatten() {
                collect_distinct_weights(child, seen, weights);
            }
        }
    }
}

trait PathBuffer<S> {
    fn push_symbol(&mut self, symbol: S);
}

impl<S> PathBuffer<S> for Vec<S> {
    fn push_symbol(&mut self, symbol: S) {
        self.push(symbol);
    }
}

impl<S, A> PathBuffer<S> for SmallVec<A>
where
    A: smallvec::Array<Item = S>,
{
    fn push_symbol(&mut self, symbol: S) {
        self.push(symbol);
    }
}

fn single_w_into<'a, S, W, B>(node: &'a WRef<S, W>, output: &mut B) -> Option<&'a W>
where
    S: Clone,
    B: PathBuffer<S>,
{
    match &node.kind {
        WKind::Shared { weight, stacks } => {
            single_u_into(stacks, output)?;
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
            output.push_symbol(top.clone());
            single_w_into(child, output)
        }
    }
}

fn single_u_into<S, B>(node: &URef<S>, output: &mut B) -> Option<()>
where
    S: Clone,
    B: PathBuffer<S>,
{
    match &node.kind {
        UKind::Segment { values, next } => {
            for value in values.iter() {
                output.push_symbol(value.clone());
            }
            single_u_into(next, output)
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
            output.push_symbol(top.clone());
            single_u_into(child, output)
        }
    }
}

fn for_each_weight_node<S, W>(node: &WRef<S, W>, visit: &mut impl FnMut(&W)) {
    fn walk<S, W>(node: &WRef<S, W>, seen: &mut FxHashSet<usize>, visit: &mut impl FnMut(&W)) {
        if !seen.insert(w_id(node)) {
            return;
        }
        match &node.kind {
            WKind::Shared { weight, .. } => visit(weight.as_ref()),
            WKind::Branch { empty, children } => {
                for weight in empty {
                    visit(weight.as_ref());
                }
                for child in children.values().flatten() {
                    walk(child, seen, visit);
                }
            }
        }
    }
    walk(node, &mut FxHashSet::default(), visit);
}

fn all_weight_nodes_satisfy<S, W>(
    node: &WRef<S, W>,
    predicate: &mut impl FnMut(&W) -> bool,
) -> bool {
    match &node.kind {
        WKind::Shared { weight, .. } => predicate(weight.as_ref()),
        WKind::Branch { empty, children } => {
            empty.iter().all(|weight| predicate(weight.as_ref()))
                && children
                    .values()
                    .flatten()
                    .all(|child| all_weight_nodes_satisfy(child, predicate))
        }
    }
}

pub(crate) fn collect_raw_paths<S, W>(
    root: &WRef<S, W>,
    max_paths: usize,
) -> Result<Vec<(Vec<S>, W)>, PathLimitExceeded>
where
    S: Clone,
    W: Clone,
{
    if root.paths > max_paths {
        return Err(PathLimitExceeded { limit: max_paths });
    }
    let mut out = Vec::with_capacity(root.paths.min(max_paths));
    let mut prefix = Vec::new();
    walk_w(root, &mut prefix, &mut |top_first, weight| {
        let mut stack = top_first.to_vec();
        stack.reverse();
        out.push((stack, weight.clone()));
    });
    Ok(out)
}

fn walk_w_bounded<S, W>(
    node: &WRef<S, W>,
    prefix: &mut SmallVec<[S; 64]>,
    limit: usize,
    emitted: &mut usize,
    visit: &mut impl FnMut(&[S], &W),
) -> bool
where
    S: Clone,
{
    match &node.kind {
        WKind::Shared { weight, stacks } => {
            walk_u_bounded(stacks, prefix, limit, emitted, &mut |path| {
                visit(path, weight)
            })
        }
        WKind::Branch { empty, children } => {
            for weight in empty {
                if *emitted == limit {
                    return false;
                }
                visit(prefix, weight);
                *emitted += 1;
            }
            for (top, values) in children {
                prefix.push(top.clone());
                for child in values {
                    if !walk_w_bounded(child, prefix, limit, emitted, visit) {
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

fn walk_u_bounded<S>(
    node: &URef<S>,
    prefix: &mut SmallVec<[S; 64]>,
    limit: usize,
    emitted: &mut usize,
    emit: &mut impl FnMut(&[S]),
) -> bool
where
    S: Clone,
{
    match &node.kind {
        UKind::Branch { empty, children } => {
            if *empty {
                if *emitted == limit {
                    return false;
                }
                emit(prefix);
                *emitted += 1;
            }
            for (top, values) in children {
                prefix.push(top.clone());
                for child in values {
                    if !walk_u_bounded(child, prefix, limit, emitted, emit) {
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
            let complete = walk_u_bounded(next, prefix, limit, emitted, emit);
            prefix.truncate(old_len);
            complete
        }
    }
}

fn walk_w<S, W>(node: &WRef<S, W>, prefix: &mut Vec<S>, emit: &mut impl FnMut(&[S], &W))
where
    S: Clone,
{
    match &node.kind {
        WKind::Shared { weight, stacks } => walk_u(stacks, prefix, &mut |path| emit(path, weight)),
        WKind::Branch { empty, children } => {
            for weight in empty {
                emit(prefix, weight);
            }
            for (top, values) in children {
                prefix.push(top.clone());
                for child in values {
                    walk_w(child, prefix, emit);
                }
                prefix.pop();
            }
        }
    }
}

fn walk_u<S>(node: &URef<S>, prefix: &mut Vec<S>, emit: &mut impl FnMut(&[S]))
where
    S: Clone,
{
    match &node.kind {
        UKind::Branch { empty, children } => {
            if *empty {
                emit(prefix);
            }
            for (top, values) in children {
                prefix.push(top.clone());
                for child in values {
                    walk_u(child, prefix, emit);
                }
                prefix.pop();
            }
        }
        UKind::Segment { values, next } => {
            let old_len = prefix.len();
            prefix.extend(values.iter().cloned());
            walk_u(next, prefix, emit);
            prefix.truncate(old_len);
        }
    }
}

pub(crate) fn structural_stats<S, W>(root: &WRef<S, W>) -> StructuralStats {
    let mut seen_w = FxHashSet::default();
    let mut seen_u = FxHashSet::default();
    let mut edges = 0usize;
    count_w(root, &mut seen_w, &mut seen_u, &mut edges);
    StructuralStats {
        nodes: seen_w.len().saturating_add(seen_u.len()),
        edges,
        paths: root.paths,
        max_depth: root.max_depth,
    }
}

fn count_w<S, W>(
    node: &WRef<S, W>,
    seen_w: &mut FxHashSet<usize>,
    seen_u: &mut FxHashSet<usize>,
    edges: &mut usize,
) {
    if !seen_w.insert(w_id(node)) {
        return;
    }
    match &node.kind {
        WKind::Shared { stacks, .. } => {
            *edges = edges.saturating_add(1);
            count_u(stacks, seen_u, edges);
        }
        WKind::Branch { children, .. } => {
            *edges = edges.saturating_add(children.values().map(SmallVec::len).sum::<usize>());
            for child in children.values().flatten() {
                count_w(child, seen_w, seen_u, edges);
            }
        }
    }
}

fn count_u<S>(node: &URef<S>, seen: &mut FxHashSet<usize>, edges: &mut usize) {
    if !seen.insert(u_id(node)) {
        return;
    }
    match &node.kind {
        UKind::Branch { children, .. } => {
            *edges = edges.saturating_add(children.values().map(SmallVec::len).sum::<usize>());
            for child in children.values().flatten() {
                count_u(child, seen, edges);
            }
        }
        UKind::Segment { next, .. } => {
            *edges = edges.saturating_add(1);
            count_u(next, seen, edges);
        }
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
        fn equivalent(&self, other: &Self) -> bool {
            self == other
        }
    }

    #[test]
    fn merging_different_weights_on_the_same_stack_dag_stays_factored() {
        let original = WeightedGss::from_stacks_with_weight(
            [vec![1_u8, 2, 3], vec![1, 2, 4], vec![1, 5]],
            Bits(1),
        );
        let mapped = original.paths().map_weights(|_| Bits(2));
        let merged = original.merge(&mapped);

        assert_eq!(merged.paths().count_at_most(usize::MAX), 3);
        assert!(
            merged
                .to_stacks(3)
                .unwrap()
                .into_iter()
                .all(|(_, weight)| weight == Bits(3))
        );
        let WKind::Shared {
            stacks: original_stacks,
            ..
        } = &original.root.kind
        else {
            panic!("homogeneous constructor must create a shared-weight root");
        };
        let WKind::Shared { stacks, .. } = &merged.root.kind else {
            panic!("same-language merge must remain a shared-weight root");
        };
        assert!(Arc::ptr_eq(original_stacks, stacks));
    }

    #[test]
    fn weight_transforms_reuse_homogeneous_stack_dag() {
        let original = WeightedGss::from_stacks_with_weight(
            [vec![1_u8, 2, 3], vec![1, 2, 4], vec![1, 5]],
            Bits(1),
        );
        let mapped = original.paths().map_weights(|_| Bits(2));
        let filtered = original.paths().filter_map_weights(|_| Some(Bits(4)));
        let WKind::Shared {
            stacks: original_stacks,
            ..
        } = &original.root.kind
        else {
            panic!("homogeneous constructor must create a shared-weight root");
        };
        for transformed in [&mapped, &filtered] {
            let WKind::Shared { stacks, .. } = &transformed.root.kind else {
                panic!("weight-only transform must keep a shared-weight root");
            };
            assert!(Arc::ptr_eq(original_stacks, stacks));
        }
    }
}
