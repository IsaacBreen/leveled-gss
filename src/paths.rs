use crate::gss::PathLimitExceeded;
use crate::nodes::{UKind, URef, WKind, WRef};

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
