use crate::Weight;
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
    pub fn path_count_at_most(&self, limit: usize) -> usize {
        self.gss.root.paths.min(limit)
    }

    /// Iterate over distinct stored weight nodes without expanding stack paths.
    ///
    /// This is representation-local: neither order nor weight placement is a
    /// semantic property of the weighted GSS.
    pub fn weights(&self) -> impl Iterator<Item = &W> {
        WeightIter::new(&self.gss.root)
    }
}

impl<'gss, S, W> Paths<'gss, S, W>
where
    S: Clone + Eq + Hash,
    W: Weight,
{
    /// Visit structural paths as top-first stack slices without materializing them.
    ///
    /// Duplicate concrete stacks may be visited more than once. At most
    /// `max_paths` callbacks are made; if more paths exist, the method then
    /// returns [`PathLimitExceeded`].
    pub fn for_each_path_top_first(
        &self,
        max_paths: usize,
        mut visit: impl FnMut(&[S], &W),
    ) -> Result<(), PathLimitExceeded> {
        let mut prefix = Vec::new();
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

    /// Write the only structural path to `output` in top-first order.
    ///
    /// Returns its stored weight, or `None` unless exactly one structural path
    /// is represented. The output buffer is cleared before use.
    pub fn write_single_path_top_first(&self, output: &mut Vec<S>) -> Option<&'gss W> {
        if self.gss.root.paths != 1 {
            return None;
        }
        output.clear();
        single_w(&self.gss.root, output)
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
        let mut u_memo = FxHashMap::default();
        let mut w_memo = FxHashMap::default();
        Ok(WeightedGss {
            root: try_map_w(&self.gss.root, &mut u_memo, &mut w_memo, &mut |weight| {
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
        let mut u_memo = FxHashMap::default();
        let mut w_memo = FxHashMap::default();
        Ok(WeightedGss {
            root: try_map_w(&self.gss.root, &mut u_memo, &mut w_memo, &mut map)?,
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

struct WeightIter<'a, S, W> {
    nodes: Vec<&'a WNode<S, W>>,
    empty_weights: Option<std::slice::Iter<'a, Arc<W>>>,
    seen: FxHashSet<usize>,
}

impl<'a, S, W> WeightIter<'a, S, W> {
    fn new(root: &'a WRef<S, W>) -> Self {
        Self {
            nodes: vec![root.as_ref()],
            empty_weights: None,
            seen: FxHashSet::default(),
        }
    }
}

impl<'a, S, W> Iterator for WeightIter<'a, S, W> {
    type Item = &'a W;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(weights) = self.empty_weights.as_mut() {
                if let Some(weight) = weights.next() {
                    return Some(weight.as_ref());
                }
                self.empty_weights = None;
            }

            let node = self.nodes.pop()?;
            let id = node as *const WNode<S, W> as usize;
            if !self.seen.insert(id) {
                continue;
            }
            match &node.kind {
                WKind::Shared { weight, .. } => return Some(weight.as_ref()),
                WKind::Branch { empty, children } => {
                    self.nodes
                        .extend(children.values().flatten().map(AsRef::as_ref));
                    self.empty_weights = Some(empty.iter());
                }
            }
        }
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

fn clone_u<S>(node: &URef<S>, memo: &mut FxHashMap<usize, URef<S>>) -> URef<S>
where
    S: Clone + Eq + Hash,
{
    let id = u_id(node);
    if let Some(cached) = memo.get(&id) {
        return cached.clone();
    }
    let cloned = match &node.kind {
        UKind::Branch { empty, children } => {
            let mut next = UChildren::default();
            for (top, values) in children {
                next.insert(
                    top.clone(),
                    values.iter().map(|child| clone_u(child, memo)).collect(),
                );
            }
            u_branch(*empty, next)
        }
        UKind::Segment { values, next } => u_segment(values.clone(), clone_u(next, memo)),
    };
    memo.insert(id, cloned.clone());
    cloned
}

fn try_map_w<S, W, V, E, F>(
    node: &WRef<S, W>,
    u_memo: &mut FxHashMap<usize, URef<S>>,
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
            Some(weight) => w_shared(Arc::new(weight), clone_u(stacks, u_memo)),
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
                    let child = try_map_w(child, u_memo, w_memo, map)?;
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

fn single_w<'a, S, W>(node: &'a WRef<S, W>, output: &mut Vec<S>) -> Option<&'a W>
where
    S: Clone,
{
    match &node.kind {
        WKind::Shared { weight, stacks } => {
            single_u(stacks, output)?;
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
            single_w(child, output)
        }
    }
}

fn single_u<S>(node: &URef<S>, output: &mut Vec<S>) -> Option<()>
where
    S: Clone,
{
    match &node.kind {
        UKind::Segment { values, next } => {
            output.extend(values.iter().cloned());
            single_u(next, output)
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
            single_u(child, output)
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
    prefix: &mut Vec<S>,
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
    prefix: &mut Vec<S>,
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
