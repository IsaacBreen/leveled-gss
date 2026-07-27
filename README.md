# weighted-gss

[![CI](https://github.com/IsaacBreen/weighted-gss/actions/workflows/ci.yml/badge.svg)](https://github.com/IsaacBreen/weighted-gss/actions/workflows/ci.yml)

A persistent weighted graph-structured stack.

`WeightedGss<S, W>` represents a finite collection of stack alternatives. Each stack carries a weight. When stack operations make alternatives denote the same concrete stack, their weights are joined.

The graph representation is private. The default Rust API is deliberately limited to construction, merging, ordinary stack operations, top selection, and bounded materialisation. High-performance parser engines can opt into a small advanced module without exposing graph internals.

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
- top selection: `top`, `tops`, `has_empty_stack`, `retain_top`, `retain_empty`, `pop_top`;
- weights: `weights`, `map_weights`, `filter_map_weights`, `joined_weight`;
- observations: `is_empty`, `max_depth`, `to_stacks`.

`to_stacks(max_paths)` returns canonical `(stack, weight)` pairs and fails rather than silently exceeding the requested structural traversal bound.

`weights()` iterates stored factored weight regions, not concrete stacks. One weight may cover many stacks, equal weights may appear more than once, and count/order are unspecified. `map_weights` and `filter_map_weights` transform those regions without materialising stacks; see [Semantics and invariants](docs/semantics.md) for the representation-independence condition.

## Optional engine API

Parser and state-machine implementations can enable a compact set of operations that avoid materialising shared stack languages:

```toml
[dependencies]
weighted-gss = { version = "0.2", features = ["engine"] }
```

The opt-in `weighted_gss::engine` module contains only:

- `for_each_stack_top_first` for bounded, allocation-light concrete-stack inspection;
- `linear_prefix` and `LinearPrefix` for mutating a homogeneous linear top prefix while retaining its hidden floor;
- `StackLanguageInterner` and `StackLanguageId` for exact fixpoint keys.

Batched parser actions, depth filters, representation IDs, structural statistics, and graph nodes remain application-local or private. See [Engine API](docs/engine.md).

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

The opt-in engine API is validated by adapting GLRMask without exposing graph nodes or restoring its historical convenience surface. The ordinary default API remains independent of parser-engine concerns.

See [Semantics and invariants](docs/semantics.md).

Licensed under either Apache-2.0 or MIT, at your option.
