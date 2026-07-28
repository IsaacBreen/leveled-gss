use crate::Weight;
use crate::segment::Segment;
use rustc_hash::{FxHashMap, FxHashSet};
use smallvec::SmallVec;
use std::hash::Hash;
use std::sync::Arc;

pub(crate) type URef<S> = Arc<UNode<S>>;
pub(crate) type WRef<S, W> = Arc<WNode<S, W>>;
pub(crate) type UChildren<S> = FxHashMap<S, SmallVec<[URef<S>; 1]>>;
pub(crate) type WChildren<S, W> = FxHashMap<S, SmallVec<[WRef<S, W>; 1]>>;

pub(crate) struct UNode<S> {
    pub(crate) kind: UKind<S>,
    pub(crate) paths: usize,
    pub(crate) max_depth: usize,
}

pub(crate) enum UKind<S> {
    Branch { empty: bool, children: UChildren<S> },
    Segment { values: Segment<S>, next: URef<S> },
}

pub(crate) struct WNode<S, W> {
    pub(crate) kind: WKind<S, W>,
    pub(crate) paths: usize,
    pub(crate) max_depth: usize,
}

pub(crate) enum WKind<S, W> {
    Branch {
        empty: SmallVec<[Arc<W>; 1]>,
        children: WChildren<S, W>,
    },
    Segment {
        values: Segment<S>,
        next: WRef<S, W>,
    },
    Shared {
        weight: Arc<W>,
        stacks: URef<S>,
    },
}

#[inline]
pub(crate) fn u_id<S>(node: &URef<S>) -> usize {
    Arc::as_ptr(node) as usize
}

#[inline]
pub(crate) fn w_id<S, W>(node: &WRef<S, W>) -> usize {
    Arc::as_ptr(node) as usize
}

pub(crate) fn u_empty<S>() -> URef<S> {
    Arc::new(UNode {
        kind: UKind::Branch {
            empty: false,
            children: UChildren::default(),
        },
        paths: 0,
        max_depth: 0,
    })
}

pub(crate) fn u_end<S>() -> URef<S> {
    Arc::new(UNode {
        kind: UKind::Branch {
            empty: true,
            children: UChildren::default(),
        },
        paths: 1,
        max_depth: 0,
    })
}

pub(crate) fn u_is_empty<S>(node: &URef<S>) -> bool {
    node.paths == 0
}

pub(crate) fn u_has_empty<S>(node: &URef<S>) -> bool {
    match &node.kind {
        UKind::Branch { empty, .. } => *empty,
        UKind::Segment { .. } => false,
    }
}

#[inline]
fn u_is_end<S>(node: &URef<S>) -> bool {
    matches!(
        &node.kind,
        UKind::Branch { empty: true, children } if children.is_empty()
    )
}

pub(crate) fn u_segment<S>(values: Segment<S>, next: URef<S>) -> URef<S>
where
    S: Clone,
{
    if values.is_empty() {
        return next;
    }
    if u_is_empty(&next) {
        return u_empty();
    }
    Arc::new(UNode {
        paths: next.paths,
        max_depth: values.len().saturating_add(next.max_depth),
        kind: UKind::Segment { values, next },
    })
}

fn dedupe_u<S>(values: &mut SmallVec<[URef<S>; 1]>) {
    let mut seen = FxHashSet::default();
    values.retain(|value| seen.insert(u_id(value)));
}

fn dedupe_w<S, W>(values: &mut SmallVec<[WRef<S, W>; 1]>) {
    let mut seen = FxHashSet::default();
    values.retain(|value| seen.insert(w_id(value)));
}

pub(crate) fn u_branch<S>(empty: bool, mut children: UChildren<S>) -> URef<S>
where
    S: Clone + Eq + Hash,
{
    children.retain(|_, values| {
        values.retain(|value| !u_is_empty(value));
        dedupe_u(values);
        !values.is_empty()
    });
    if !empty && children.is_empty() {
        return u_empty();
    }
    if !empty && children.len() == 1 {
        let (top, children) = children.into_iter().next().expect("one child key");
        let rest = u_merge_all(children);
        return u_segment(Segment::one(top), rest);
    }
    let paths = children
        .values()
        .flatten()
        .fold(usize::from(empty), |n, child| n.saturating_add(child.paths));
    let max_depth = children
        .values()
        .flatten()
        .map(|child| child.max_depth.saturating_add(1))
        .max()
        .unwrap_or(0);
    Arc::new(UNode {
        kind: UKind::Branch { empty, children },
        paths,
        max_depth,
    })
}

fn u_after_prefix<S>(node: &URef<S>, count: usize) -> URef<S>
where
    S: Clone + Eq + Hash,
{
    if count == 0 {
        return node.clone();
    }
    match &node.kind {
        UKind::Segment { values, next } => {
            if count < values.len() {
                u_segment(
                    values.drop_front(count).expect("count is inside segment"),
                    next.clone(),
                )
            } else {
                u_after_prefix(next, count - values.len())
            }
        }
        UKind::Branch { .. } => u_popn(node, count),
    }
}

pub(crate) fn u_merge<S>(left: &URef<S>, right: &URef<S>) -> URef<S>
where
    S: Clone + Eq + Hash,
{
    if Arc::ptr_eq(left, right) {
        return left.clone();
    }
    if u_is_empty(left) {
        return right.clone();
    }
    if u_is_empty(right) {
        return left.clone();
    }
    if u_is_end(left) && u_is_end(right) {
        return left.clone();
    }
    if let (
        UKind::Segment {
            values: left_values,
            ..
        },
        UKind::Segment {
            values: right_values,
            ..
        },
    ) = (&left.kind, &right.kind)
    {
        let common = left_values
            .iter()
            .zip(right_values.iter())
            .take_while(|(a, b)| a == b)
            .count();
        if common > 0 {
            let prefix =
                Segment::from_top_first(left_values.iter().take(common).cloned().collect());
            let left_rest = u_after_prefix(left, common);
            let right_rest = u_after_prefix(right, common);
            return u_segment(prefix, u_merge(&left_rest, &right_rest));
        }
    }

    let mut empty = false;
    let mut children = UChildren::default();
    for node in [left, right] {
        match &node.kind {
            UKind::Branch {
                empty: node_empty,
                children: node_children,
            } => {
                empty |= *node_empty;
                for (top, values) in node_children {
                    children
                        .entry(top.clone())
                        .or_default()
                        .extend(values.iter().cloned());
                }
            }
            UKind::Segment { values, .. } => {
                let top = values.first().expect("segments are non-empty").clone();
                children
                    .entry(top)
                    .or_default()
                    .push(u_after_prefix(node, 1));
            }
        }
    }
    u_branch(empty, children)
}

pub(crate) fn u_merge_all<S>(values: impl IntoIterator<Item = URef<S>>) -> URef<S>
where
    S: Clone + Eq + Hash,
{
    let mut queue: std::collections::VecDeque<_> = values.into_iter().collect();
    if queue.is_empty() {
        return u_empty();
    }
    while queue.len() > 1 {
        let mut next = std::collections::VecDeque::with_capacity(queue.len().div_ceil(2));
        while let Some(left) = queue.pop_front() {
            if let Some(right) = queue.pop_front() {
                next.push_back(u_merge(&left, &right));
            } else {
                next.push_back(left);
            }
        }
        queue = next;
    }
    queue.pop_front().expect("non-empty queue")
}

struct USinglePathCursor<'a, S> {
    node: &'a URef<S>,
    segment_index: usize,
    finished: bool,
}

impl<'a, S> USinglePathCursor<'a, S> {
    fn new(node: &'a URef<S>) -> Self {
        Self {
            node,
            segment_index: 0,
            finished: false,
        }
    }

    fn next(&mut self) -> Option<&'a S> {
        if self.finished {
            return None;
        }
        loop {
            match &self.node.kind {
                UKind::Segment { values, next } => {
                    if let Some(value) = values.get(self.segment_index) {
                        self.segment_index += 1;
                        return Some(value);
                    }
                    self.node = next;
                    self.segment_index = 0;
                }
                UKind::Branch { empty, children } => {
                    if *empty {
                        debug_assert!(children.is_empty());
                        self.finished = true;
                        return None;
                    }
                    let mut alternatives = children
                        .iter()
                        .flat_map(|(top, values)| values.iter().map(move |child| (top, child)));
                    let (top, child) = alternatives
                        .next()
                        .expect("a non-empty single path has one child");
                    debug_assert!(alternatives.next().is_none());
                    self.node = child;
                    self.segment_index = 0;
                    return Some(top);
                }
            }
        }
    }
}

fn u_same_single_path<S>(left: &URef<S>, right: &URef<S>) -> bool
where
    S: Eq,
{
    if Arc::ptr_eq(left, right) {
        return true;
    }
    if left.paths != 1 || right.paths != 1 || left.max_depth != right.max_depth {
        return false;
    }

    let mut left = USinglePathCursor::new(left);
    let mut right = USinglePathCursor::new(right);
    loop {
        match (left.next(), right.next()) {
            (Some(left), Some(right)) if left == right => {}
            (None, None) => return true,
            _ => return false,
        }
    }
}

fn u_common_single_path_prefix<S>(left: &URef<S>, right: &URef<S>) -> Vec<S>
where
    S: Clone + Eq,
{
    if left.paths != 1 || right.paths != 1 {
        return Vec::new();
    }
    let mut left = USinglePathCursor::new(left);
    let mut right = USinglePathCursor::new(right);
    let mut prefix = Vec::new();
    while let (Some(left), Some(right)) = (left.next(), right.next()) {
        if left != right {
            break;
        }
        prefix.push(left.clone());
    }
    prefix
}

pub(crate) fn u_push<S>(node: &URef<S>, value: S) -> URef<S>
where
    S: Clone,
{
    if u_is_empty(node) {
        return node.clone();
    }
    u_segment(Segment::one(value), node.clone())
}

pub(crate) fn u_pop<S>(node: &URef<S>) -> URef<S>
where
    S: Clone + Eq + Hash,
{
    match &node.kind {
        UKind::Segment { .. } => u_after_prefix(node, 1),
        UKind::Branch { children, .. } => {
            u_merge_all(children.values().flatten().cloned().collect::<Vec<_>>())
        }
    }
}

pub(crate) fn u_popn<S>(node: &URef<S>, count: usize) -> URef<S>
where
    S: Clone + Eq + Hash,
{
    if count == 0 || u_is_empty(node) {
        return node.clone();
    }
    match &node.kind {
        UKind::Segment { values, next } => {
            if count < values.len() {
                u_segment(
                    values.drop_front(count).expect("count is inside segment"),
                    next.clone(),
                )
            } else {
                u_popn(next, count - values.len())
            }
        }
        UKind::Branch { children, .. } => u_merge_all(
            children
                .values()
                .flatten()
                .map(|child| u_popn(child, count - 1))
                .collect::<Vec<_>>(),
        ),
    }
}

pub(crate) fn u_tops<S>(node: &URef<S>) -> SmallVec<[S; 8]>
where
    S: Clone,
{
    match &node.kind {
        UKind::Segment { values, .. } => values.first().cloned().into_iter().collect(),
        UKind::Branch { children, .. } => children.keys().cloned().collect(),
    }
}

pub(crate) fn u_single_exclusive_top<S>(node: &URef<S>) -> Option<S>
where
    S: Clone,
{
    match &node.kind {
        UKind::Segment { values, .. } => values.first().cloned(),
        UKind::Branch { empty, children } => {
            if *empty || children.len() != 1 {
                return None;
            }
            children.keys().next().cloned()
        }
    }
}

pub(crate) fn u_retain_top<S>(node: &URef<S>, top: &S) -> URef<S>
where
    S: Clone + Eq + Hash,
{
    match &node.kind {
        UKind::Segment { values, .. } => {
            if values.first() == Some(top) {
                node.clone()
            } else {
                u_empty()
            }
        }
        UKind::Branch { children, .. } => {
            let mut kept = UChildren::default();
            if let Some(values) = children.get(top) {
                kept.insert(top.clone(), values.clone());
            }
            u_branch(false, kept)
        }
    }
}

pub(crate) fn u_pop_top<S>(node: &URef<S>, top: &S) -> URef<S>
where
    S: Clone + Eq + Hash,
{
    match &node.kind {
        UKind::Segment { values, .. } => {
            if values.first() == Some(top) {
                u_after_prefix(node, 1)
            } else {
                u_empty()
            }
        }
        UKind::Branch { children, .. } => children
            .get(top)
            .map_or_else(u_empty, |values| u_merge_all(values.iter().cloned())),
    }
}

pub(crate) fn u_retain_empty<S>(node: &URef<S>) -> URef<S> {
    if u_has_empty(node) {
        u_end()
    } else {
        u_empty()
    }
}

pub(crate) fn w_empty<S, W>() -> WRef<S, W> {
    Arc::new(WNode {
        kind: WKind::Branch {
            empty: SmallVec::new(),
            children: WChildren::default(),
        },
        paths: 0,
        max_depth: 0,
    })
}

pub(crate) fn w_is_empty<S, W>(node: &WRef<S, W>) -> bool {
    node.paths == 0
}

pub(crate) fn w_shared<S, W>(weight: Arc<W>, stacks: URef<S>) -> WRef<S, W> {
    if u_is_empty(&stacks) {
        return w_empty();
    }
    Arc::new(WNode {
        paths: stacks.paths,
        max_depth: stacks.max_depth,
        kind: WKind::Shared { weight, stacks },
    })
}

pub(crate) fn w_segment<S, W>(values: Segment<S>, next: WRef<S, W>) -> WRef<S, W>
where
    S: Clone,
{
    if values.is_empty() {
        return next;
    }
    if w_is_empty(&next) {
        return w_empty();
    }

    if let WKind::Segment {
        values: following,
        next: floor,
    } = &next.kind
    {
        let mut combined = Vec::with_capacity(values.len().saturating_add(following.len()));
        combined.extend(values.iter().cloned());
        combined.extend(following.iter().cloned());
        return Arc::new(WNode {
            paths: floor.paths,
            max_depth: combined.len().saturating_add(floor.max_depth),
            kind: WKind::Segment {
                values: Segment::from_top_first(combined),
                next: floor.clone(),
            },
        });
    }

    Arc::new(WNode {
        paths: next.paths,
        max_depth: values.len().saturating_add(next.max_depth),
        kind: WKind::Segment { values, next },
    })
}

fn w_after_prefix<S, W>(node: &WRef<S, W>, count: usize) -> WRef<S, W>
where
    S: Clone + Eq + Hash,
    W: Weight,
{
    if count == 0 {
        return node.clone();
    }
    match &node.kind {
        WKind::Segment { values, next } => {
            if count < values.len() {
                w_segment(
                    values.drop_front(count).expect("count is inside segment"),
                    next.clone(),
                )
            } else {
                w_after_prefix(next, count - values.len())
            }
        }
        _ => w_popn(node, count),
    }
}

pub(crate) fn w_branch<S, W>(
    mut empty: SmallVec<[Arc<W>; 1]>,
    mut children: WChildren<S, W>,
) -> WRef<S, W>
where
    S: Clone + Eq + Hash,
{
    let mut seen_empty = FxHashSet::default();
    empty.retain(|weight| seen_empty.insert(Arc::as_ptr(weight) as usize));
    children.retain(|_, values| {
        values.retain(|value| !w_is_empty(value));
        dedupe_w(values);
        !values.is_empty()
    });
    if empty.is_empty() && children.is_empty() {
        return w_empty();
    }
    let paths = children
        .values()
        .flatten()
        .fold(empty.len(), |n, child| n.saturating_add(child.paths));
    let max_depth = children
        .values()
        .flatten()
        .map(|child| child.max_depth.saturating_add(1))
        .max()
        .unwrap_or(0);
    Arc::new(WNode {
        kind: WKind::Branch { empty, children },
        paths,
        max_depth,
    })
}

fn w_expose_top<S, W>(node: &WRef<S, W>) -> WRef<S, W>
where
    S: Clone + Eq + Hash,
    W: Weight,
{
    match &node.kind {
        WKind::Branch { .. } => node.clone(),
        WKind::Segment { values, .. } => {
            let top = values.first().expect("segments are non-empty").clone();
            let mut children = WChildren::default();
            children
                .entry(top)
                .or_default()
                .push(w_after_prefix(node, 1));
            w_branch(SmallVec::new(), children)
        }
        WKind::Shared { weight, stacks } => match &stacks.kind {
            UKind::Segment { values, .. } => {
                let top = values.first().expect("segments are non-empty").clone();
                let mut children = WChildren::default();
                children
                    .entry(top)
                    .or_default()
                    .push(w_shared(weight.clone(), u_after_prefix(stacks, 1)));
                w_branch(SmallVec::new(), children)
            }
            UKind::Branch { empty, children } => {
                let mut weighted_empty = SmallVec::new();
                if *empty {
                    weighted_empty.push(weight.clone());
                }
                let mut weighted_children = WChildren::default();
                for (top, values) in children {
                    for child in values {
                        weighted_children
                            .entry(top.clone())
                            .or_default()
                            .push(w_shared(weight.clone(), child.clone()));
                    }
                }
                w_branch(weighted_empty, weighted_children)
            }
        },
    }
}

fn w_top_segment<S, W>(node: &WRef<S, W>) -> Option<&Segment<S>> {
    match &node.kind {
        WKind::Segment { values, .. } => Some(values),
        WKind::Shared { stacks, .. } => match &stacks.kind {
            UKind::Segment { values, .. } => Some(values),
            UKind::Branch { .. } => None,
        },
        WKind::Branch { .. } => None,
    }
}

pub(crate) fn w_merge<S, W>(left: &WRef<S, W>, right: &WRef<S, W>) -> WRef<S, W>
where
    S: Clone + Eq + Hash,
    W: Weight,
{
    let mut memo = FxHashMap::default();
    let mut exposed_keepalive = Vec::new();
    w_merge_memo(left, right, &mut memo, &mut exposed_keepalive)
}

fn w_merge_memo<S, W>(
    left: &WRef<S, W>,
    right: &WRef<S, W>,
    memo: &mut FxHashMap<(usize, usize), WRef<S, W>>,
    exposed_keepalive: &mut Vec<WRef<S, W>>,
) -> WRef<S, W>
where
    S: Clone + Eq + Hash,
    W: Weight,
{
    if Arc::ptr_eq(left, right) {
        return left.clone();
    }
    if w_is_empty(left) {
        return right.clone();
    }
    if w_is_empty(right) {
        return left.clone();
    }

    let left_id = w_id(left);
    let right_id = w_id(right);
    let key = if left_id <= right_id {
        (left_id, right_id)
    } else {
        (right_id, left_id)
    };
    if let Some(cached) = memo.get(&key) {
        return cached.clone();
    }

    if matches!(left.kind, WKind::Segment { .. }) || matches!(right.kind, WKind::Segment { .. }) {
        if let (Some(left_values), Some(right_values)) = (w_top_segment(left), w_top_segment(right))
        {
            let common = left_values
                .iter()
                .zip(right_values.iter())
                .take_while(|(left, right)| left == right)
                .count();
            if common > 0 {
                let prefix =
                    Segment::from_top_first(left_values.iter().take(common).cloned().collect());
                let left_rest = w_after_prefix(left, common);
                let right_rest = w_after_prefix(right, common);
                exposed_keepalive.push(left_rest.clone());
                exposed_keepalive.push(right_rest.clone());
                let floor = w_merge_memo(&left_rest, &right_rest, memo, exposed_keepalive);
                let result = w_segment(prefix, floor);
                memo.insert(key, result.clone());
                return result;
            }
        }
    }

    if let (
        WKind::Shared {
            weight: left_weight,
            stacks: left_stacks,
        },
        WKind::Shared {
            weight: right_weight,
            stacks: right_stacks,
        },
    ) = (&left.kind, &right.kind)
    {
        if Arc::ptr_eq(left_stacks, right_stacks) || u_same_single_path(left_stacks, right_stacks) {
            let weight = if Arc::ptr_eq(left_weight, right_weight)
                || left_weight.as_ref() == right_weight.as_ref()
            {
                left_weight.clone()
            } else {
                Arc::new(left_weight.join(right_weight.as_ref()))
            };
            let result = w_shared(weight, left_stacks.clone());
            memo.insert(key, result.clone());
            return result;
        }
        if left_stacks.paths == 1 && right_stacks.paths == 1 {
            let prefix = u_common_single_path_prefix(left_stacks, right_stacks);
            if !prefix.is_empty() {
                let count = prefix.len();
                let left_rest = w_shared(left_weight.clone(), u_popn(left_stacks, count));
                let right_rest = w_shared(right_weight.clone(), u_popn(right_stacks, count));
                exposed_keepalive.push(left_rest.clone());
                exposed_keepalive.push(right_rest.clone());
                let floor = w_merge_memo(&left_rest, &right_rest, memo, exposed_keepalive);
                let result = w_segment(Segment::from_top_first(prefix), floor);
                memo.insert(key, result.clone());
                return result;
            }
        }
        if Arc::ptr_eq(left_weight, right_weight) || left_weight.as_ref() == right_weight.as_ref() {
            let result = w_shared(left_weight.clone(), u_merge(left_stacks, right_stacks));
            memo.insert(key, result.clone());
            return result;
        }
    }

    let exposed_left = w_expose_top(left);
    let exposed_right = w_expose_top(right);
    // Exposing a shared unweighted frontier creates temporary weighted child
    // nodes. Memo keys use pointer identity, so retain those synthetic roots
    // until the whole merge finishes and allocator addresses cannot be reused.
    if !Arc::ptr_eq(&exposed_left, left) {
        exposed_keepalive.push(exposed_left.clone());
    }
    if !Arc::ptr_eq(&exposed_right, right) {
        exposed_keepalive.push(exposed_right.clone());
    }
    let left = exposed_left;
    let right = exposed_right;
    let (
        WKind::Branch {
            empty: left_empty,
            children: left_children,
        },
        WKind::Branch {
            empty: right_empty,
            children: right_children,
        },
    ) = (&left.kind, &right.kind)
    else {
        unreachable!()
    };

    let empty = join_weight_arcs(left_empty.iter().chain(right_empty.iter()));
    let mut all_children = WChildren::default();
    for (top, values) in left_children.iter().chain(right_children.iter()) {
        all_children
            .entry(top.clone())
            .or_default()
            .extend(values.iter().cloned());
    }

    let mut children = WChildren::default();
    for (top, values) in all_children {
        let mut values = values.into_iter();
        let Some(mut merged) = values.next() else {
            continue;
        };
        for value in values {
            merged = w_merge_memo(&merged, &value, memo, exposed_keepalive);
        }
        children.entry(top).or_default().push(merged);
    }

    let result = w_branch(empty, children);
    memo.insert(key, result.clone());
    result
}

fn join_weight_arcs<'a, W>(weights: impl IntoIterator<Item = &'a Arc<W>>) -> SmallVec<[Arc<W>; 1]>
where
    W: Weight + 'a,
{
    let mut weights = weights.into_iter();
    let Some(first) = weights.next() else {
        return SmallVec::new();
    };
    let mut result = first.clone();
    for weight in weights {
        if Arc::ptr_eq(&result, weight) || result.as_ref() == weight.as_ref() {
            continue;
        }
        result = Arc::new(result.join(weight.as_ref()));
    }
    SmallVec::from_buf([result])
}

pub(crate) fn w_merge_all<S, W>(values: impl IntoIterator<Item = WRef<S, W>>) -> WRef<S, W>
where
    S: Clone + Eq + Hash,
    W: Weight,
{
    let mut queue: std::collections::VecDeque<_> = values.into_iter().collect();
    if queue.is_empty() {
        return w_empty();
    }
    while queue.len() > 1 {
        let mut next = std::collections::VecDeque::with_capacity(queue.len().div_ceil(2));
        while let Some(left) = queue.pop_front() {
            if let Some(right) = queue.pop_front() {
                next.push_back(w_merge(&left, &right));
            } else {
                next.push_back(left);
            }
        }
        queue = next;
    }
    queue.pop_front().expect("non-empty queue")
}

pub(crate) fn w_push<S, W>(node: &WRef<S, W>, value: S) -> WRef<S, W>
where
    S: Clone + Eq + Hash,
{
    if w_is_empty(node) {
        return node.clone();
    }
    match &node.kind {
        WKind::Shared { weight, stacks } => w_shared(weight.clone(), u_push(stacks, value)),
        WKind::Branch { .. } | WKind::Segment { .. } => {
            w_segment(Segment::one(value), node.clone())
        }
    }
}

pub(crate) fn w_pop<S, W>(node: &WRef<S, W>) -> WRef<S, W>
where
    S: Clone + Eq + Hash,
    W: Weight,
{
    match &node.kind {
        WKind::Shared { weight, stacks } => w_shared(weight.clone(), u_pop(stacks)),
        WKind::Segment { .. } => w_after_prefix(node, 1),
        WKind::Branch { children, .. } => {
            w_merge_all(children.values().flatten().cloned().collect::<Vec<_>>())
        }
    }
}

pub(crate) fn w_popn<S, W>(node: &WRef<S, W>, count: usize) -> WRef<S, W>
where
    S: Clone + Eq + Hash,
    W: Weight,
{
    if count == 0 {
        return node.clone();
    }
    match &node.kind {
        WKind::Shared { weight, stacks } => w_shared(weight.clone(), u_popn(stacks, count)),
        WKind::Segment { .. } => w_after_prefix(node, count),
        WKind::Branch { children, .. } => w_merge_all(
            children
                .values()
                .flatten()
                .map(|child| w_popn(child, count - 1))
                .collect::<Vec<_>>(),
        ),
    }
}

pub(crate) fn w_tops<S, W>(node: &WRef<S, W>) -> SmallVec<[S; 8]>
where
    S: Clone + Eq + Hash,
{
    match &node.kind {
        WKind::Shared { stacks, .. } => u_tops(stacks),
        WKind::Segment { values, .. } => values.first().cloned().into_iter().collect(),
        WKind::Branch { children, .. } => children.keys().cloned().collect(),
    }
}

pub(crate) fn w_single_exclusive_top<S, W>(node: &WRef<S, W>) -> Option<S>
where
    S: Clone + Eq + Hash,
{
    match &node.kind {
        WKind::Shared { stacks, .. } => u_single_exclusive_top(stacks),
        WKind::Segment { values, .. } => values.first().cloned(),
        WKind::Branch { empty, children } => {
            if !empty.is_empty() || children.len() != 1 {
                return None;
            }
            children.keys().next().cloned()
        }
    }
}

pub(crate) fn w_has_empty<S, W>(node: &WRef<S, W>) -> bool {
    match &node.kind {
        WKind::Shared { stacks, .. } => u_has_empty(stacks),
        WKind::Segment { .. } => false,
        WKind::Branch { empty, .. } => !empty.is_empty(),
    }
}

pub(crate) fn w_retain_top<S, W>(node: &WRef<S, W>, top: &S) -> WRef<S, W>
where
    S: Clone + Eq + Hash,
{
    match &node.kind {
        WKind::Shared { weight, stacks } => w_shared(weight.clone(), u_retain_top(stacks, top)),
        WKind::Segment { values, .. } => {
            if values.first() == Some(top) {
                node.clone()
            } else {
                w_empty()
            }
        }
        WKind::Branch { children, .. } => {
            let mut kept = WChildren::default();
            if let Some(values) = children.get(top) {
                kept.insert(top.clone(), values.clone());
            }
            w_branch(SmallVec::new(), kept)
        }
    }
}

pub(crate) fn w_pop_top<S, W>(node: &WRef<S, W>, top: &S) -> WRef<S, W>
where
    S: Clone + Eq + Hash,
    W: Weight,
{
    match &node.kind {
        WKind::Shared { weight, stacks } => w_shared(weight.clone(), u_pop_top(stacks, top)),
        WKind::Segment { values, .. } => {
            if values.first() == Some(top) {
                w_after_prefix(node, 1)
            } else {
                w_empty()
            }
        }
        WKind::Branch { children, .. } => children
            .get(top)
            .map_or_else(w_empty, |values| w_merge_all(values.iter().cloned())),
    }
}

pub(crate) fn w_retain_empty<S, W>(node: &WRef<S, W>) -> WRef<S, W>
where
    S: Clone + Eq + Hash,
{
    match &node.kind {
        WKind::Shared { weight, stacks } => w_shared(weight.clone(), u_retain_empty(stacks)),
        WKind::Segment { .. } => w_empty(),
        WKind::Branch { empty, .. } => w_branch(empty.clone(), WChildren::default()),
    }
}

pub(crate) fn w_joined_weight<S, W>(node: &WRef<S, W>) -> Option<W>
where
    W: Weight,
{
    fn walk<S, W>(node: &WRef<S, W>, seen: &mut FxHashSet<usize>, out: &mut Option<W>)
    where
        W: Weight,
    {
        if !seen.insert(w_id(node)) {
            return;
        }
        match &node.kind {
            WKind::Shared { weight, .. } => join_into(out, weight.as_ref()),
            WKind::Segment { next, .. } => walk(next, seen, out),
            WKind::Branch { empty, children } => {
                for weight in empty {
                    join_into(out, weight.as_ref());
                }
                for child in children.values().flatten() {
                    walk(child, seen, out);
                }
            }
        }
    }
    let mut seen = FxHashSet::default();
    let mut out = None;
    walk(node, &mut seen, &mut out);
    out
}

#[cfg(feature = "python")]
pub(crate) fn w_empty_weight<S, W>(node: &WRef<S, W>) -> Option<W>
where
    W: Weight,
{
    let mut out = None;
    match &node.kind {
        WKind::Shared { weight, stacks } if u_has_empty(stacks) => {
            out = Some(weight.as_ref().clone())
        }
        WKind::Branch { empty, .. } => {
            for weight in empty {
                join_into(&mut out, weight.as_ref());
            }
        }
        WKind::Segment { .. } => {}
        _ => {}
    }
    out
}

pub(crate) fn join_into<W: Weight>(out: &mut Option<W>, value: &W) {
    *out = Some(match out.take() {
        Some(current) => current.join(value),
        None => value.clone(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merging_terminal_suffixes_reuses_one_node() {
        let left = u_end::<u8>();
        let right = u_end::<u8>();

        let merged = u_merge(&left, &right);

        assert!(Arc::ptr_eq(&merged, &left));
    }

    #[test]
    fn separately_segmented_equal_single_paths_compare_equal() {
        let end = u_end();
        let left = u_segment(Segment::from_top_first(vec![3_u8, 2, 1]), end.clone());
        let right = u_segment(
            Segment::one(3_u8),
            u_segment(Segment::from_top_first(vec![2_u8, 1]), end),
        );
        let different = u_segment(Segment::from_top_first(vec![3_u8, 9, 1]), u_end());

        assert!(u_same_single_path(&left, &right));
        assert!(!u_same_single_path(&left, &different));
    }
}
