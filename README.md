# weighted-gss

[![CI](https://github.com/IsaacBreen/weighted-gss/actions/workflows/ci.yml/badge.svg)](https://github.com/IsaacBreen/weighted-gss/actions/workflows/ci.yml)
[![Fuzz](https://github.com/IsaacBreen/weighted-gss/actions/workflows/fuzz.yml/badge.svg)](https://github.com/IsaacBreen/weighted-gss/actions/workflows/fuzz.yml)

A persistent weighted graph-structured stack.

`WeightedGss<S, W>` represents a finite collection of stack alternatives. Each stack carries a weight. When stack operations make alternatives denote the same concrete stack, their weights are joined.

The graph representation is private. The Rust API contains semantic stack operations, bounded concrete-stack inspection, and a linear-prefix fast path without exposing graph nodes or canonical representation machinery.

## Installation

The latest release is version 0.2.2.

```toml
[dependencies]
weighted-gss = "0.2.2"
```

Python 3.8 or later:

```bash
python -m pip install "weighted-gss==0.2.2"
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

The exported Rust names are:

```rust
Weight
WeightedGss
Gss
LinearPrefix
StackLimitExceeded
linear_prefix
for_each_stack_top_first
```

The core methods are:

- construction: `new`, `from_stack`, `from_stacks`, `from_stacks_with_weight`;
- alternatives: `merge`;
- stack operations: `push`, `pop`, `popn`;
- top selection: `top`, `tops`, `has_empty_stack`, `retain_top`, `retain_empty`, `pop_top`;
- weights: `weights`, `map_weights`, `filter_map_weights`, `joined_weight`;
- observations: `is_empty`, `max_depth`, `to_stacks`.

`to_stacks(max_stacks)` returns canonical `(stack, weight)` pairs and fails with the opaque `StackLimitExceeded` error rather than returning more than the requested number of distinct stacks.

`weights()` iterates stored factored weight regions, not concrete stacks. One weight may cover many stacks, equal weights may appear more than once, and count/order are unspecified. `map_weights` and `filter_map_weights` transform those regions without materialising stacks; see [Semantics and invariants](docs/semantics.md) for the representation-independence condition.

## Bounded inspection and linear prefixes

`for_each_stack_top_first(&gss, max_stacks, visit)` visits canonical distinct stacks as borrowed top-first slices. It completes only when the complete result fits within `max_stacks`.

`linear_prefix(&gss)` returns a `LinearPrefix` when the current value has one homogeneous weight and a directly accessible linear top prefix. The hidden floor may still branch. The view supports indexed reads from the top, pushes, bounded pops, and conversion back into a `WeightedGss` while retaining the unchanged floor.

Neither operation exposes graph nodes, structural paths, or canonical stack-language IDs.

## Validation and performance characteristics

Correctness is checked against an explicit stack-to-weight map through deterministic tests, shrinkable property-based operation sequences, and an oracle-backed `cargo-fuzz` target. Benchmark baselines implement the same extensional semantics with explicit maps or sets, and the structural benchmark builders are themselves checked for identical outputs.

Representative Criterion point estimates from one clean Apple M1 Pro run are shown below. Input preparation and output destruction are excluded from construction timings. These are measurements of particular public operation traces, not machine-independent guarantees.

| Workload | `WeightedGss` | Explicit map | What it shows |
|---|---:|---:|---|
| Grow a 65,536-stack binary language through 16 rounds of push and merge, preserving two weight classes | 12.5 µs | 26.6 ms | Structural evolution remains compact while the explicit population doubles each round |
| Pop a structurally built 4,096-stack binary language | 104 ns | 235 µs | A shared graph operation need not enumerate the represented stacks |
| Fork a persistent 512-stack value twice | 163 ns | 101 µs | Immutable structure is reused rather than copied |
| Import an already enumerated list of 1,024 weighted stacks | 3.57 ms | 15.1 µs | Conversion from the explicit representation naturally favours the explicit map |
| Materialise 1,024 concrete stacks as owned output | 86.9 µs | 36.5 µs | Complete output remains proportional to the concrete language |

The distinction between **structural evolution** and **flat import or enumeration** is fundamental. `WeightedGss` is intended to preserve and update shared stack structure; it is not a faster hash map for inputs that have already been fully expanded.

```bash
cargo test --all-targets
cargo bench
cargo +nightly fuzz run operation_sequences
```

See [Correctness validation](docs/validation.md), [Benchmark methodology](docs/benchmarks.md), and the [2026-07-28 benchmark audit](https://github.com/IsaacBreen/weighted-gss/blob/main/docs/validation/benchmark-audit-2026-07-28.md).

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

## Semantics

Weights and stack operations have an extensional meaning independent of the private graph representation. Rust 1.85 is the declared minimum version. See [Semantics and invariants](docs/semantics.md).

Licensed under either Apache-2.0 or MIT, at your option.
