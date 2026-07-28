use crate::Weight;
use crate::gss::WeightedGss;
use crate::nodes::*;
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;
use std::hash::Hash;
use std::sync::Arc;

pub(crate) fn iter<S, W>(gss: &WeightedGss<S, W>) -> impl Iterator<Item = &W> {
    WeightIter::new(&gss.root)
}

pub(crate) fn filter_map<S, W, V>(
    gss: &WeightedGss<S, W>,
    mut transform: impl FnMut(&W) -> Option<V>,
) -> WeightedGss<S, V>
where
    S: Clone + Eq + Hash,
    W: Weight,
    V: Weight,
{
    let mut memo = FxHashMap::default();
    WeightedGss {
        root: map_node_weights(&gss.root, &mut memo, &mut transform),
    }
}

struct WeightIter<'a, S, W> {
    root: &'a WRef<S, W>,
    nodes: SmallVec<[&'a WNode<S, W>; 16]>,
    empty_weights: Option<std::slice::Iter<'a, Arc<W>>>,
    seen: NodeSeen,
    started: bool,
}

#[derive(Default)]
struct NodeSeen {
    inline: SmallVec<[usize; 16]>,
    overflow: Option<FxHashSet<usize>>,
}

impl NodeSeen {
    fn insert(&mut self, id: usize) -> bool {
        if let Some(seen) = self.overflow.as_mut() {
            return seen.insert(id);
        }
        if self.inline.contains(&id) {
            return false;
        }
        if self.inline.len() < self.inline.inline_size() {
            self.inline.push(id);
            return true;
        }
        let mut seen = FxHashSet::with_capacity_and_hasher(
            self.inline.len().saturating_mul(2),
            Default::default(),
        );
        seen.extend(self.inline.iter().copied());
        let inserted = seen.insert(id);
        self.overflow = Some(seen);
        inserted
    }
}

impl<'a, S, W> WeightIter<'a, S, W> {
    fn new(root: &'a WRef<S, W>) -> Self {
        let mut nodes = SmallVec::new();
        nodes.push(root.as_ref());
        Self {
            root,
            nodes,
            empty_weights: None,
            seen: NodeSeen::default(),
            started: false,
        }
    }
}

impl<'a, S, W> Iterator for WeightIter<'a, S, W> {
    type Item = &'a W;

    fn next(&mut self) -> Option<Self::Item> {
        self.started = true;
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
                WKind::Segment { next, .. } => self.nodes.push(next.as_ref()),
                WKind::Branch { empty, children } => {
                    self.nodes
                        .extend(children.values().flatten().map(AsRef::as_ref));
                    self.empty_weights = Some(empty.iter());
                }
            }
        }
    }

    fn all<F>(&mut self, mut predicate: F) -> bool
    where
        Self: Sized,
        F: FnMut(Self::Item) -> bool,
    {
        if !self.started {
            self.started = true;
            self.nodes.clear();
            self.empty_weights = None;
            return all_weights_satisfy(self.root, &mut predicate);
        }
        self.by_ref().all(predicate)
    }

    fn for_each<F>(mut self, mut visit: F)
    where
        Self: Sized,
        F: FnMut(Self::Item),
    {
        if !self.started {
            self.started = true;
            visit_weights(self.root, &mut visit);
            return;
        }
        for weight in self {
            visit(weight);
        }
    }
}

fn visit_weights<'a, S, W>(node: &'a WRef<S, W>, visit: &mut impl FnMut(&'a W)) {
    fn walk<'a, S, W>(node: &'a WRef<S, W>, seen: &mut NodeSeen, visit: &mut impl FnMut(&'a W)) {
        if !seen.insert(w_id(node)) {
            return;
        }
        match &node.kind {
            WKind::Shared { weight, .. } => visit(weight.as_ref()),
            WKind::Segment { next, .. } => walk(next, seen, visit),
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
    walk(node, &mut NodeSeen::default(), visit);
}

fn all_weights_satisfy<'a, S, W>(
    node: &'a WRef<S, W>,
    predicate: &mut impl FnMut(&'a W) -> bool,
) -> bool {
    fn walk<'a, S, W>(
        node: &'a WRef<S, W>,
        seen: &mut NodeSeen,
        predicate: &mut impl FnMut(&'a W) -> bool,
    ) -> bool {
        if !seen.insert(w_id(node)) {
            return true;
        }
        match &node.kind {
            WKind::Shared { weight, .. } => predicate(weight.as_ref()),
            WKind::Segment { next, .. } => walk(next, seen, predicate),
            WKind::Branch { empty, children } => {
                empty.iter().all(|weight| predicate(weight.as_ref()))
                    && children
                        .values()
                        .flatten()
                        .all(|child| walk(child, seen, predicate))
            }
        }
    }
    walk(node, &mut NodeSeen::default(), predicate)
}

fn map_node_weights<S, W, V>(
    node: &WRef<S, W>,
    memo: &mut FxHashMap<usize, WRef<S, V>>,
    transform: &mut impl FnMut(&W) -> Option<V>,
) -> WRef<S, V>
where
    S: Clone + Eq + Hash,
    W: Weight,
    V: Weight,
{
    let id = w_id(node);
    if let Some(cached) = memo.get(&id) {
        return cached.clone();
    }
    let mapped = match &node.kind {
        WKind::Shared { weight, stacks } => transform(weight.as_ref())
            .map_or_else(w_empty, |weight| w_shared(Arc::new(weight), stacks.clone())),
        WKind::Segment { values, next } => {
            let next = map_node_weights(next, memo, transform);
            w_segment(values.clone(), next)
        }
        WKind::Branch { empty, children } => {
            let mapped_empty = empty
                .iter()
                .filter_map(|weight| transform(weight.as_ref()).map(Arc::new))
                .collect();
            let mut mapped_children = WChildren::default();
            for (top, values) in children {
                for child in values {
                    let child = map_node_weights(child, memo, transform);
                    if !w_is_empty(&child) {
                        mapped_children.entry(top.clone()).or_default().push(child);
                    }
                }
            }
            w_branch(mapped_empty, mapped_children)
        }
    };
    memo.insert(id, mapped.clone());
    mapped
}
