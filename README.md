# weighted-gss

[![CI](https://github.com/IsaacBreen/weighted-gss/actions/workflows/ci.yml/badge.svg)](https://github.com/IsaacBreen/weighted-gss/actions/workflows/ci.yml)

A persistent weighted graph-structured stack for nondeterministic stack machines.

A `WeightedGss<S, W>` stores a collection of stack alternatives. Every stack has a weight, and when operations make alternatives denote the same concrete stack, their weights are joined. Common stack tails are shared, while linear regions use compact segments and can be exposed through a mutable fast-path view.

The public API is expressed entirely in terms of stacks, alternatives, weights, and stack operations. The graph representation is private.

## Installation

Rust:

```bash
cargo add weighted-gss
```

Python 3.8 or later:

```bash
python -m pip install weighted-gss
```

The registries currently contain version 0.1.0, which exposes the earlier extracted API. The redesigned API documented here will be released as 0.2.0. Until then, use this Git branch or repository head:

```toml
[dependencies]
weighted-gss = { git = "https://github.com/IsaacBreen/weighted-gss", branch = "rewrite/from-scratch-20260725" }
```

```bash
python -m pip install "git+https://github.com/IsaacBreen/weighted-gss@rewrite/from-scratch-20260725"
```

## Rust use

Stacks are supplied and returned bottom-to-top.

```rust
use weighted_gss::{StackOp, Weight, WeightedGss};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Possibilities(u32);

impl Weight for Possibilities {
    fn join(&self, other: &Self) -> Self {
        Self(self.0 | other.0)
    }
}

let left = WeightedGss::from_stack([0_u32, 1, 2], Possibilities(0b001));
let right = WeightedGss::from_stack([0_u32, 1, 3], Possibilities(0b100));
let stacks = left.merge(&right);

assert_eq!(stacks.top(), None);
assert_eq!(stacks.tops().collect::<std::collections::BTreeSet<_>>(), [2, 3].into());

let reduced = stacks.pop_top(&2).push(9);
assert_eq!(
    reduced.to_stacks(8).unwrap(),
    vec![(vec![0, 1, 9], Possibilities(0b001))],
);

let shifted = stacks.apply_top_ops([
    (2, StackOp::new(1, [8])),
    (3, StackOp::new(0, [9])),
]);
assert_eq!(shifted.max_depth(), 4);
```

`Weight` must implement ordinary equality. `Weight::join` must be associative, commutative, and idempotent.

## Python use

The Python binding exposes the ordinary semantic API:

```python
from dataclasses import dataclass
from weighted_gss import WeightedGSS

@dataclass(frozen=True)
class Possibilities:
    bits: int

    def join(self, other: "Possibilities") -> "Possibilities":
        return Possibilities(self.bits | other.bits)

stacks = WeightedGSS.from_stacks([
    ([0, 1, 2], Possibilities(0b001)),
    ([0, 1, 3], Possibilities(0b100)),
])

assert stacks.tops() == {2, 3}
assert stacks.pop_top(2).to_stacks() == [
    ([0, 1], Possibilities(0b001)),
]
```

Python weights need not be hashable. Exceptions raised by `join()` are propagated normally. See the [Python API](docs/python.md).

## Core operations

Construction and alternatives:

- `new`, `from_stack`, `from_stacks`, `from_stacks_with_weight`, `with_stack`
- `merge`, `merge_all`

Stack operations:

- `push`, `pop`, `popn`
- `top`, `tops`, `retain_top`, `retain_empty`, `pop_top`, `pop_branches`
- `retain_where_at_depth`
- `apply_op`, `apply_ops`, `apply_top_ops`

Observations:

- `is_empty`, `max_depth`, `has_empty_stack`
- `joined_weight`, `empty_weight`
- bounded, canonical `to_stacks`

## Path-local operations

Some algorithms need to transform the weights attached to the currently stored paths rather than first joining every concrete stack. That boundary is explicit:

```rust
let pruned = stacks.paths().filter_map_weights(|weight| {
    (weight.0 != 0).then_some(*weight)
});
```

`paths()` also provides bounded structural traversal, path counts, immutable weight iteration, weight partitioning, and an allocation-free single-path callback. Structural layout is not part of the API contract. See [Semantics](docs/semantics.md).

## Linear fast path

When the top of a GSS is a linear segment, `try_virtual_stack()` exposes it as a mutable `VirtualStack`:

```rust
if let Some(mut stack) = stacks.try_virtual_stack() {
    if stack.pop_prefix(2) == 0 {
        stack.push(7);
        let stacks = stack.into_gss();
        // Continue with the general representation when needed.
        drop(stacks);
    }
}
```

A virtual stack may sit above a branched hidden floor. `is_complete()` distinguishes a complete concrete stack from a linear prefix over such a floor.

## Exact stack-language keys

Fixpoint algorithms can use `StackLanguageInterner` to obtain exact compact IDs for the unweighted concrete stack language. The IDs ignore weights, segment boundaries, sharing layout, and duplicate representation paths.

## Design and validation

The API is sufficient to implement GLRMask without accessing graph internals. A compatibility adapter built only from this public API passes GLRMask's complete serial Rust library suite: 855 tests passed, 2 ignored.

The standalone crate additionally validates operations against an explicit stack-to-weight map, tests linear prefixes over branched floors, and checks exact stack-language interning on a DAG representing 262,144 stacks. The ABI3 Python wheel is tested against the same semantic model, including callback-exception propagation and strict type checking.

See:

- [Semantics and invariants](docs/semantics.md)
- [Advanced facilities](docs/advanced.md)
- [Contributing](CONTRIBUTING.md)

Licensed under either Apache-2.0 or MIT, at your option.
