# leveled-gss

[![CI](https://github.com/IsaacBreen/leveled-gss/actions/workflows/ci.yml/badge.svg)](https://github.com/IsaacBreen/leveled-gss/actions/workflows/ci.yml)

A persistent, weighted graph-structured stack with leveled sharing, implemented in Rust with Python bindings.

`leveled-gss` represents a set of stacks as one immutable graph. Common suffixes are shared; branching and merging preserve path-specific accumulators; and deterministic prefixes can be manipulated through a mutable `VirtualStack` fast path. The implementation was extracted from the data structure used by [GLRMask](https://github.com/IsaacBreen/glrmask).

The API is experimental and follows semantic versioning. Version 0.1 is suitable for evaluation and integration, but may receive breaking API changes in later 0.x releases.

## Installation

Rust:

```bash
cargo add leveled-gss
```

Python 3.8 or later:

```bash
python -m pip install leveled-gss
```

Install the unreleased Git head with either Cargo or pip:

```toml
[dependencies]
leveled-gss = { git = "https://github.com/IsaacBreen/leveled-gss" }
```

```bash
python -m pip install "git+https://github.com/IsaacBreen/leveled-gss"
```

## Rust example

Stacks are passed bottom-to-top.

```rust
use leveled_gss::{LeveledGSS, Merge};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Cost(u32);

impl Merge for Cost {
    fn merge(&self, other: &Self) -> Self {
        Cost(self.0.min(other.0))
    }
}

let left = LeveledGSS::from_single_stack(vec![0_u32, 1, 2], Cost(7));
let right = LeveledGSS::from_single_stack(vec![0_u32, 1, 3], Cost(4));
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

`Merge` is a join operation, not an arbitrary reduction. It must be associative, commutative, and idempotent. Set union, bitwise OR, minimum, and maximum are valid examples. Addition generally is not.

The Rust API reference is generated on [docs.rs](https://docs.rs/leveled-gss).

## Python example

Unweighted use stores `None` as the accumulator:

```python
from leveled_gss import LeveledGSS

gss = LeveledGSS.from_unweighted([[0, 1, 2], [0, 1, 3]])
pushed = gss.push(9)

assert {tuple(stack) for stack, _ in pushed.to_stacks()} == {
    (0, 1, 2, 9),
    (0, 1, 3, 9),
}
```

Weighted accumulators must be immutable and hashable and must define `merge(other)`:

```python
from dataclasses import dataclass
from leveled_gss import LeveledGSS

@dataclass(frozen=True)
class Bits:
    value: int

    def merge(self, other: "Bits") -> "Bits":
        return Bits(self.value | other.value)

gss = LeveledGSS.from_stacks([
    ([0, 1], Bits(0b001)),
    ([0, 1], Bits(0b100)),
])

assert gss.to_stacks() == [([0, 1], Bits(0b101))]
```

The Python package is typed (`py.typed`) and exposes runtime docstrings through `help(LeveledGSS)`. See the [Python API guide](docs/python.md).

## Semantics

- Operations are persistent: inputs remain valid and new values retain structural sharing where possible.
- `push(value)` pushes onto every active path.
- `popn(n)` discards paths shorter than `n`; a path of length exactly `n` becomes an empty stack.
- Equivalent concrete stacks have their accumulators joined.
- `to_stacks(limit)` is a bounded diagnostic operation and never silently truncates.
- Structurally distinct graph paths can denote the same concrete stack. Consequently, `path_count_at_most` counts structural paths, not necessarily unique value sequences.

See [Semantics and invariants](docs/semantics.md) for the complete contract.

## Main types

- `LeveledGSS<T, A>`: persistent shared collection of weighted stack paths.
- `VirtualStack<T, A>`: mutable view of a deterministic stack prefix.
- `Merge`: accumulator join at convergent paths.
- `LeveledGSSSummary`: structural diagnostics without path materialization.

## Testing

The repository tests:

- the production regression suite inherited from GLRMask;
- both segment backends (`vec` and `arc`);
- 40,000 randomized operation steps against an explicit stack-set model;
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
