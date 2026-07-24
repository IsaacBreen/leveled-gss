# weighted-gss

[![CI](https://github.com/IsaacBreen/weighted-gss/actions/workflows/ci.yml/badge.svg)](https://github.com/IsaacBreen/weighted-gss/actions/workflows/ci.yml)

A persistent weighted graph-structured stack, implemented in Rust with Python bindings.

`weighted-gss` represents a finite map from complete stacks to weights. Stack suffixes are shared in a compact graph, and whenever stack operations make two stacks identical, their weights are joined. The implementation uses leveled sharing, weight-free shared suffixes, compact deterministic segments, and persistent path copying.

The implementation was extracted from the graph-structured stack used by [GLRMask](https://github.com/IsaacBreen/glrmask). Version 0.1 is suitable for evaluation and integration; later 0.x releases may make breaking API changes.

## Installation

Rust:

```bash
cargo add weighted-gss
```

Python 3.8 or later:

```bash
python -m pip install weighted-gss
```

Install the unreleased Git head:

```toml
[dependencies]
weighted-gss = { git = "https://github.com/IsaacBreen/weighted-gss" }
```

```bash
python -m pip install "git+https://github.com/IsaacBreen/weighted-gss"
```

## Rust example

Stacks are ordered bottom-to-top.

```rust
use weighted_gss::{Weight, WeightedGss};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Cost(u32);

impl Weight for Cost {
    fn join(&self, other: &Self) -> Self {
        Cost(self.0.min(other.0))
    }
}

let left = WeightedGss::from_single_stack(vec![0_u32, 1, 2], Cost(7));
let right = WeightedGss::from_single_stack(vec![0_u32, 1, 3], Cost(4));
let gss = left.merge(&right).push(9);

let mut stacks = gss.to_stacks(8).expect("materialization limit exceeded");
stacks.sort_by(|a, b| a.0.cmp(&b.0));
assert_eq!(
    stacks,
    vec![
        (vec![0, 1, 2, 9], Cost(7)),
        (vec![0, 1, 3, 9], Cost(4)),
    ],
);
```

`Weight::join` must be associative, commutative, and idempotent. Set union, bitwise OR, minimum, and maximum are valid examples. Addition generally is not.

The Rust API reference is generated on [docs.rs](https://docs.rs/weighted-gss).

## Python example

Unweighted use stores `None` as the weight:

```python
from weighted_gss import WeightedGSS

gss = WeightedGSS.from_unweighted([[0, 1, 2], [0, 1, 3]])
pushed = gss.push(9)

assert {tuple(stack) for stack, _ in pushed.to_stacks()} == {
    (0, 1, 2, 9),
    (0, 1, 3, 9),
}
```

Weighted values must be immutable and hashable and must define `join(other)`:

```python
from dataclasses import dataclass
from weighted_gss import WeightedGSS

@dataclass(frozen=True)
class Bits:
    value: int

    def join(self, other: "Bits") -> "Bits":
        return Bits(self.value | other.value)

gss = WeightedGSS.from_stacks([
    ([0, 1], Bits(0b001)),
    ([0, 1], Bits(0b100)),
])

assert gss.to_stacks() == [([0, 1], Bits(0b101))]
```

The Python distribution is typed (`py.typed`) and provides runtime docstrings. See the [Python API guide](docs/python.md).

## Semantics

- A value denotes a finite map from bottom-to-top stacks to weights.
- Operations are persistent: inputs remain valid and results retain structural sharing where possible.
- `push(value)` pushes onto every represented stack.
- `popn(n)` discards stacks shorter than `n`; stacks of length exactly `n` become empty stacks.
- When two represented stacks become identical, their weights are joined.
- `to_stacks(limit)` is bounded and never silently truncates.
- `path_count_at_most` counts structural graph paths, which can exceed the number of distinct stack keys.

See [Semantics and invariants](docs/semantics.md) for the complete contract.

## Main types

- `WeightedGss<T, W>`: persistent compressed map from stacks to weights.
- `Weight`: join operation for weights on coincident stacks.
- `VirtualStack<T, W>`: mutable fast path for a deterministic stack prefix.
- `WeightedGssSummary`: structural diagnostics without path materialization.

The Python equivalents are `WeightedGSS` and `WeightedGSSSummary`.

## Testing

The repository tests:

- the production regression suite inherited from GLRMask;
- both segment backends (`vec` and `arc`);
- 40,000 randomized operation steps against an explicit stack-to-weight map;
- a compressed graph representing 262,144 stacks;
- Rust examples and doctests;
- Python weighted and unweighted APIs from built wheels and source distributions;
- package metadata and publication dry runs;
- Linux, macOS, and Windows in GitHub Actions.

## Provenance

The initial standalone extraction tracks `glrmask` commit `58c24ff44e3a796172a0ea532b3d66affa188d9e`. The standalone crate changes the inherited parser-floor underflow behavior so `popn` follows ordinary stack semantics.

## Contributing and license

See [CONTRIBUTING.md](CONTRIBUTING.md) for development instructions.

Licensed under either the Apache License, Version 2.0 or the MIT License, at your option.
