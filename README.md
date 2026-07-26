# weighted-gss

[![CI](https://github.com/IsaacBreen/weighted-gss/actions/workflows/ci.yml/badge.svg)](https://github.com/IsaacBreen/weighted-gss/actions/workflows/ci.yml)

A persistent weighted graph-structured stack.

`WeightedGss<S, W>` represents a finite collection of stack alternatives. Each stack carries a weight. When stack operations make alternatives denote the same concrete stack, their weights are joined.

The graph representation is private. The Rust API is deliberately limited to construction, merging, ordinary stack operations, top selection, and bounded materialisation.

## Installation

The registries currently contain version 0.1.0. The redesigned API documented here is being prepared as 0.2.0.

```toml
[dependencies]
weighted-gss = { git = "https://github.com/IsaacBreen/weighted-gss", branch = "rewrite/from-scratch-20260725" }
```

Python 3.8 or later:

```bash
python -m pip install "git+https://github.com/IsaacBreen/weighted-gss@rewrite/from-scratch-20260725"
```

## Rust

Stacks are supplied and returned bottom-to-top.

```rust
use weighted_gss::{Weight, WeightedGss};

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
assert_eq!(
    stacks.tops().collect::<std::collections::BTreeSet<_>>(),
    [2, 3].into(),
);

let reduced = stacks.pop_top(&2).push(9);
assert_eq!(
    reduced.to_stacks(8).unwrap(),
    vec![(vec![0, 1, 9], Possibilities(0b001))],
);
```

A weight must implement ordinary equality. `join` must be associative, commutative, and idempotent.

The exported Rust names are only:

```rust
Weight
WeightedGss
Gss
PathLimitExceeded
```

The core methods are:

- construction: `new`, `from_stack`, `from_stacks`, `from_stacks_with_weight`;
- alternatives: `merge`;
- stack operations: `push`, `pop`, `popn`;
- top selection: `top`, `tops`, `has_empty_stack`, `retain_top`, `pop_top`;
- observations: `is_empty`, `max_depth`, `joined_weight`, `to_stacks`.

`to_stacks(max_paths)` returns canonical `(stack, weight)` pairs and fails rather than silently exceeding the requested structural traversal bound.

## Python

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

## Semantics and validation

The implementation is tested against an explicit stack-to-weight map under randomized sequences of construction, merge, push, pop, top selection, and branch selection. Rust 1.85 is the declared minimum version.

A previous broader candidate was also used to implement and benchmark GLRMask as an implementation stress test. Parser-specific traversal, virtual-stack, operation-batching, and language-interning facilities exercised by that adapter are intentionally **not** part of the 0.2 public API.

See [Semantics and invariants](docs/semantics.md).

Licensed under either Apache-2.0 or MIT, at your option.
