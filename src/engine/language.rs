use crate::gss::WeightedGss;
use crate::nodes::{UKind, URef, WKind, WRef, u_id, w_id};
use rustc_hash::{FxHashMap, FxHashSet};
use std::hash::Hash;

/// Opaque canonical identifier for an extensional set of concrete stacks.
///
/// An ID is meaningful only with the [`StackLanguageInterner`] that returned it.
/// IDs from different interner instances must not be compared; unrelated
/// interners may assign the same token to different stack languages.
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
/// not affect the returned ID. IDs are exact within one interner instance and
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

    /// Intern the exact stack language of `gss` after erasing its weights.
    ///
    /// The returned ID is local to this interner instance.
    pub fn intern<W>(&mut self, gss: &WeightedGss<S, W>) -> StackLanguageId {
        const MAX_RECURSIVE_DEPTH: usize = 256;

        if let WKind::Shared { stacks, .. } = &gss.root.kind {
            return self.unweighted_key_auto(stacks);
        }

        let mut weighted_memo = std::mem::take(&mut self.weighted_scratch);
        weighted_memo.clear();
        let id = if gss.root.max_depth <= MAX_RECURSIVE_DEPTH {
            self.weighted_key(&gss.root, &mut weighted_memo)
        } else {
            self.weighted_key_iterative(&gss.root, &mut weighted_memo)
        };
        self.weighted_scratch = weighted_memo;
        id
    }

    fn intern_node(
        &mut self,
        empty: bool,
        mut children: Vec<(S, StackLanguageId)>,
    ) -> StackLanguageId {
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
        // Trie nodes are interned bottom-up, so an ID is an upper bound on
        // path depth. Keep the small common case recursive and switch large
        // canonical tries to a worklist before call-stack depth becomes risky.
        const MAX_RECURSIVE_ID: u32 = 1_024;

        if left == right || right.0 == 0 {
            return left;
        }
        if left.0 == 0 {
            return right;
        }
        let key = Self::union_pair(left, right);
        if let Some(id) = self.union_memo.get(&key) {
            return *id;
        }
        if left.0.max(right.0) > MAX_RECURSIVE_ID {
            return self.union_iterative(left, right);
        }
        self.union_recursive(left, right)
    }

    #[inline]
    fn union_pair(left: StackLanguageId, right: StackLanguageId) -> (u32, u32) {
        if left.0 <= right.0 {
            (left.0, right.0)
        } else {
            (right.0, left.0)
        }
    }

    fn union_recursive(
        &mut self,
        left: StackLanguageId,
        right: StackLanguageId,
    ) -> StackLanguageId {
        let key = Self::union_pair(left, right);
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
        let id = self.intern_node(left_node.empty || right_node.empty, children);
        self.union_memo.insert(key, id);
        id
    }

    fn union_iterative(
        &mut self,
        left: StackLanguageId,
        right: StackLanguageId,
    ) -> StackLanguageId {
        let root = Self::union_pair(left, right);
        if let Some(id) = self.union_memo.get(&root) {
            return *id;
        }

        let mut pending = vec![root];
        let mut unseen = FxHashSet::default();
        let mut order = Vec::new();
        while let Some(pair @ (left, right)) = pending.pop() {
            if left == right || left == 0 || right == 0 || self.union_memo.contains_key(&pair) {
                continue;
            }
            if !unseen.insert(pair) {
                continue;
            }
            order.push(pair);

            let left_children = &self.nodes[left as usize].children;
            let right_children = &self.nodes[right as usize].children;
            let mut li = 0;
            let mut ri = 0;
            while li < left_children.len() && ri < right_children.len() {
                match left_children[li].0.cmp(&right_children[ri].0) {
                    std::cmp::Ordering::Less => li += 1,
                    std::cmp::Ordering::Greater => ri += 1,
                    std::cmp::Ordering::Equal => {
                        let child_pair =
                            Self::union_pair(left_children[li].1, right_children[ri].1);
                        if child_pair.0 != child_pair.1
                            && child_pair.0 != 0
                            && !self.union_memo.contains_key(&child_pair)
                        {
                            pending.push(child_pair);
                        }
                        li += 1;
                        ri += 1;
                    }
                }
            }
        }

        order.sort_unstable_by_key(|(left, right)| (*left).max(*right));

        for pair @ (left, right) in order {
            if self.union_memo.contains_key(&pair) {
                continue;
            }
            let left_node = self.nodes[left as usize].clone();
            let right_node = self.nodes[right as usize].clone();
            let mut children =
                Vec::with_capacity(left_node.children.len() + right_node.children.len());
            let mut li = 0;
            let mut ri = 0;
            while li < left_node.children.len() || ri < right_node.children.len() {
                if ri == right_node.children.len()
                    || (li < left_node.children.len()
                        && left_node.children[li].0 < right_node.children[ri].0)
                {
                    children.push(left_node.children[li].clone());
                    li += 1;
                } else if li == left_node.children.len()
                    || right_node.children[ri].0 < left_node.children[li].0
                {
                    children.push(right_node.children[ri].clone());
                    ri += 1;
                } else {
                    let left_child = left_node.children[li].1;
                    let right_child = right_node.children[ri].1;
                    let child = if left_child == right_child {
                        left_child
                    } else if left_child.0 == 0 {
                        right_child
                    } else if right_child.0 == 0 {
                        left_child
                    } else {
                        *self
                            .union_memo
                            .get(&Self::union_pair(left_child, right_child))
                            .expect("stack-language child union must precede its parent")
                    };
                    children.push((left_node.children[li].0.clone(), child));
                    li += 1;
                    ri += 1;
                }
            }
            let id = self.intern_node(left_node.empty || right_node.empty, children);
            self.union_memo.insert(pair, id);
        }

        *self
            .union_memo
            .get(&root)
            .expect("stack-language root union was not constructed")
    }

    fn unweighted_key_auto(&mut self, root: &URef<S>) -> StackLanguageId {
        const MAX_RECURSIVE_DEPTH: usize = 256;

        if root.max_depth <= MAX_RECURSIVE_DEPTH {
            self.unweighted_key(root)
        } else {
            self.unweighted_key_iterative(root)
        }
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
                self.intern_node(*empty, canonical)
            }
            UKind::Segment { values, next } => {
                let mut child = self.unweighted_key(next);
                for symbol in values.iter().rev() {
                    child = self.intern_node(false, vec![(symbol.clone(), child)]);
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
                self.intern_node(!empty.is_empty(), canonical)
            }
        };
        memo.insert(pointer, id);
        id
    }

    fn unweighted_key_iterative(&mut self, root: &URef<S>) -> StackLanguageId {
        if let Some(id) = self.unweighted_memo.get(&u_id(root)) {
            return *id;
        }

        let mut pending = vec![root.clone()];
        let mut seen = FxHashSet::default();
        let mut nodes = Vec::new();
        while let Some(node) = pending.pop() {
            let pointer = u_id(&node);
            if self.unweighted_memo.contains_key(&pointer) || !seen.insert(pointer) {
                continue;
            }
            match &node.kind {
                UKind::Segment { next, .. } => pending.push(next.clone()),
                UKind::Branch { children, .. } => {
                    pending.extend(children.values().flatten().cloned())
                }
            }
            nodes.push(node);
        }

        nodes.sort_unstable_by_key(|node| node.max_depth);
        for node in nodes {
            let pointer = u_id(&node);
            if self.unweighted_memo.contains_key(&pointer) {
                continue;
            }
            let id = match &node.kind {
                UKind::Segment { values, next } => {
                    let mut id = *self
                        .unweighted_memo
                        .get(&u_id(next))
                        .expect("stack-language segment child must precede its parent");
                    for symbol in values.iter().rev() {
                        id = self.intern_node(false, vec![(symbol.clone(), id)]);
                    }
                    id
                }
                UKind::Branch { empty, children } => {
                    let mut canonical = Vec::with_capacity(children.len());
                    for (symbol, alternatives) in children {
                        let mut child = StackLanguageId(0);
                        for alternative in alternatives {
                            let next = *self
                                .unweighted_memo
                                .get(&u_id(alternative))
                                .expect("stack-language branch child must precede its parent");
                            child = self.union(child, next);
                        }
                        if child.0 != 0 {
                            canonical.push((symbol.clone(), child));
                        }
                    }
                    self.intern_node(*empty, canonical)
                }
            };
            self.unweighted_memo.insert(pointer, id);
            self.unweighted_keepalive.push(node);
        }

        *self
            .unweighted_memo
            .get(&u_id(root))
            .expect("stack-language unweighted root must be canonicalized")
    }

    fn weighted_key_iterative<W>(
        &mut self,
        root: &WRef<S, W>,
        weighted_memo: &mut FxHashMap<usize, StackLanguageId>,
    ) -> StackLanguageId {
        let mut pending = vec![root.clone()];
        let mut seen = FxHashSet::default();
        let mut nodes = Vec::new();

        while let Some(node) = pending.pop() {
            let pointer = w_id(&node);
            if weighted_memo.contains_key(&pointer) || !seen.insert(pointer) {
                continue;
            }
            match &node.kind {
                WKind::Shared { stacks, .. } => {
                    self.unweighted_key_auto(stacks);
                }
                WKind::Branch { children, .. } => {
                    pending.extend(children.values().flatten().cloned());
                }
            }
            nodes.push(node);
        }

        nodes.sort_unstable_by_key(|node| node.max_depth);
        for node in nodes {
            let pointer = w_id(&node);
            if weighted_memo.contains_key(&pointer) {
                continue;
            }
            let id = match &node.kind {
                WKind::Shared { stacks, .. } => *self
                    .unweighted_memo
                    .get(&u_id(stacks))
                    .expect("stack-language shared support must precede its parent"),
                WKind::Branch { empty, children } => {
                    let mut canonical = Vec::with_capacity(children.len());
                    for (symbol, alternatives) in children {
                        let mut child = StackLanguageId(0);
                        for alternative in alternatives {
                            let next = *weighted_memo
                                .get(&w_id(alternative))
                                .expect("stack-language weighted child must precede its parent");
                            child = self.union(child, next);
                        }
                        if child.0 != 0 {
                            canonical.push((symbol.clone(), child));
                        }
                    }
                    self.intern_node(!empty.is_empty(), canonical)
                }
            };
            weighted_memo.insert(pointer, id);
        }

        *weighted_memo
            .get(&w_id(root))
            .expect("stack-language weighted root must be canonicalized")
    }
}
