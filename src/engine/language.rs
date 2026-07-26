use crate::gss::WeightedGss;
use crate::nodes::{UKind, URef, WKind, WRef, u_id, w_id};
use rustc_hash::FxHashMap;
use std::hash::Hash;

/// Opaque canonical identifier for an extensional set of concrete stacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StackLanguageId(u32);

#[derive(Clone, PartialEq, Eq, Hash)]
struct TrieNode<S> {
    empty: bool,
    children: Vec<(S, StackLanguageId)>,
}

/// Canonicalizes the unweighted stack language denoted by weighted GSS values.
///
/// Weights, structural path duplication, segment boundaries, and DAG layout do
/// not affect the returned key. IDs are exact within one interner instance and
/// are intended for visited sets in fixpoint algorithms.
pub struct StackLanguageInterner<S> {
    nodes: Vec<TrieNode<S>>,
    interned: FxHashMap<TrieNode<S>, StackLanguageId>,
    unweighted_memo: FxHashMap<usize, StackLanguageId>,
    unweighted_keepalive: Vec<URef<S>>,
    weighted_scratch: FxHashMap<usize, StackLanguageId>,
    union_memo: FxHashMap<(u32, u32), StackLanguageId>,
}

impl<S> Default for StackLanguageInterner<S>
where
    S: Clone + Eq + Hash + Ord,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<S> StackLanguageInterner<S>
where
    S: Clone + Eq + Hash + Ord,
{
    /// Construct an empty interner.
    #[must_use]
    pub fn new() -> Self {
        let empty = TrieNode {
            empty: false,
            children: Vec::new(),
        };
        let mut interned = FxHashMap::default();
        interned.insert(empty.clone(), StackLanguageId(0));
        Self {
            nodes: vec![empty],
            interned,
            unweighted_memo: FxHashMap::default(),
            unweighted_keepalive: Vec::new(),
            weighted_scratch: FxHashMap::default(),
            union_memo: FxHashMap::default(),
        }
    }

    /// Return the exact canonical key of `gss` after erasing path weights.
    pub fn key<W>(&mut self, gss: &WeightedGss<S, W>) -> StackLanguageId {
        let mut weighted_memo = std::mem::take(&mut self.weighted_scratch);
        weighted_memo.clear();
        let id = self.weighted_key(&gss.root, &mut weighted_memo);
        self.weighted_scratch = weighted_memo;
        id
    }

    fn intern(&mut self, empty: bool, mut children: Vec<(S, StackLanguageId)>) -> StackLanguageId {
        children.retain(|(_, child)| child.0 != 0);
        children.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        debug_assert!(children.windows(2).all(|pair| pair[0].0 != pair[1].0));
        let node = TrieNode { empty, children };
        if let Some(id) = self.interned.get(&node) {
            return *id;
        }
        let id = StackLanguageId(
            u32::try_from(self.nodes.len()).expect("stack-language interner exceeded u32 IDs"),
        );
        self.nodes.push(node.clone());
        self.interned.insert(node, id);
        id
    }

    fn union(&mut self, left: StackLanguageId, right: StackLanguageId) -> StackLanguageId {
        if left == right || right.0 == 0 {
            return left;
        }
        if left.0 == 0 {
            return right;
        }
        let key = if left.0 <= right.0 {
            (left.0, right.0)
        } else {
            (right.0, left.0)
        };
        if let Some(id) = self.union_memo.get(&key) {
            return *id;
        }

        let left_node = self.nodes[left.0 as usize].clone();
        let right_node = self.nodes[right.0 as usize].clone();
        let mut children = Vec::with_capacity(left_node.children.len() + right_node.children.len());
        let mut li = 0;
        let mut ri = 0;
        while li < left_node.children.len() || ri < right_node.children.len() {
            match (left_node.children.get(li), right_node.children.get(ri)) {
                (Some((left_symbol, left_child)), Some((right_symbol, right_child))) => {
                    match left_symbol.cmp(right_symbol) {
                        std::cmp::Ordering::Less => {
                            children.push((left_symbol.clone(), *left_child));
                            li += 1;
                        }
                        std::cmp::Ordering::Greater => {
                            children.push((right_symbol.clone(), *right_child));
                            ri += 1;
                        }
                        std::cmp::Ordering::Equal => {
                            let child = self.union(*left_child, *right_child);
                            children.push((left_symbol.clone(), child));
                            li += 1;
                            ri += 1;
                        }
                    }
                }
                (Some((symbol, child)), None) => {
                    children.push((symbol.clone(), *child));
                    li += 1;
                }
                (None, Some((symbol, child))) => {
                    children.push((symbol.clone(), *child));
                    ri += 1;
                }
                (None, None) => break,
            }
        }
        let id = self.intern(left_node.empty || right_node.empty, children);
        self.union_memo.insert(key, id);
        id
    }

    fn unweighted_key(&mut self, node: &URef<S>) -> StackLanguageId {
        let pointer = u_id(node);
        if let Some(id) = self.unweighted_memo.get(&pointer) {
            return *id;
        }
        let id = match &node.kind {
            UKind::Branch { empty, children } => {
                let mut canonical = Vec::with_capacity(children.len());
                for (symbol, alternatives) in children {
                    let child = alternatives
                        .iter()
                        .fold(StackLanguageId(0), |current, next| {
                            let next = self.unweighted_key(next);
                            self.union(current, next)
                        });
                    if child.0 != 0 {
                        canonical.push((symbol.clone(), child));
                    }
                }
                self.intern(*empty, canonical)
            }
            UKind::Segment { values, next } => {
                let mut child = self.unweighted_key(next);
                for symbol in values.iter().rev() {
                    child = self.intern(false, vec![(symbol.clone(), child)]);
                }
                child
            }
        };
        self.unweighted_memo.insert(pointer, id);
        self.unweighted_keepalive.push(node.clone());
        id
    }

    fn weighted_key<W>(
        &mut self,
        node: &WRef<S, W>,
        memo: &mut FxHashMap<usize, StackLanguageId>,
    ) -> StackLanguageId {
        let pointer = w_id(node);
        if let Some(id) = memo.get(&pointer) {
            return *id;
        }
        let id = match &node.kind {
            WKind::Shared { stacks, .. } => self.unweighted_key(stacks),
            WKind::Branch { empty, children } => {
                let mut canonical = Vec::with_capacity(children.len());
                for (symbol, alternatives) in children {
                    let child = alternatives
                        .iter()
                        .fold(StackLanguageId(0), |current, next| {
                            let next = self.weighted_key(next, memo);
                            self.union(current, next)
                        });
                    if child.0 != 0 {
                        canonical.push((symbol.clone(), child));
                    }
                }
                self.intern(!empty.is_empty(), canonical)
            }
        };
        memo.insert(pointer, id);
        id
    }
}
