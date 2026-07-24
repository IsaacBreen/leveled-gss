# leveled-gss

A persistent, weighted graph-structured stack with leveled sharing, written in Rust with Python bindings.

The crate represents many stacks in one immutable graph. Common suffixes are shared, branching and merging preserve path-specific accumulators, and deterministic prefixes can be manipulated through a mutable `VirtualStack` fast path. The implementation was extracted from the data structure used by [GLRMask](https://github.com/IsaacBreen/glrmask).

The API is experimental. The standalone crate preserves the production representation while giving ordinary stack operations ordinary semantics: `popn(n)` discards paths shorter than `n`.

## Rust

Until the crate is published on crates.io:

```toml
[dependencies]
leveled-gss = { git = "https://github.com/IsaacBreen/leveled-gss" }
```

```rust
use leveled_gss::{LeveledGSS, Merge};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Cost(u32);

impl Merge for Cost {
    fn merge(&self, other: &Self) -> Self {
        Cost(self.0.min(other.0))
    }
}

let a = LeveledGSS::from_single_stack(vec![0_u32, 1, 2], Cost(7));
let b = LeveledGSS::from_single_stack(vec![0_u32, 1, 3], Cost(4));
let stacks = a.merge(&b).push(9);

assert_eq!(stacks.path_count_at_most(10), 2);
assert_eq!(stacks.peek(), [9].into_iter().collect());
```

`Merge` is a join operation, not an arbitrary reduction. It should be associative, commutative, and idempotent. Examples include set union, bitwise OR, minimum, and maximum.

## Python

The Python extension is built with PyO3 and supports Python 3.8 or later through the stable ABI.

Install directly from GitHub:

```bash
python -m pip install "git+https://github.com/IsaacBreen/leveled-gss"
```

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

Weighted accumulators must be hashable and define `merge(other)`:

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

Python exposes construction, bounded materialization, push, pop, isolation, merge, fuse, top inspection, accumulator reduction, path counting, and structural summaries.

## Main types

- `LeveledGSS<T, A>`: persistent shared collection of weighted stack paths.
- `VirtualStack<T, A>`: mutable view of a deterministic stack prefix.
- `Merge`: accumulator join at convergent paths.
- `LeveledGSSSummary`: structural diagnostics without enumerating all paths.

`LeveledGSS::to_stacks` materializes graph paths only up to an explicit limit. It is useful for tests and inspection, but it is deliberately not the main execution model. Structurally distinct graph paths can denote the same concrete stack, so `path_count_at_most` is a graph-path count rather than a unique-stack count.

## Testing

The repository tests:

- the original production regression suite;
- both segment backends (`vec` and `arc`);
- 40,000 randomized operation sequences against an explicit stack-set model;
- a compressed graph representing 262,144 stacks;
- Rust examples and doctests;
- Python weighted and unweighted APIs from a built wheel;
- Linux, macOS, and Windows in GitHub Actions.

## Provenance

The initial standalone extraction tracks `glrmask` commit `58c24ff44e3a796172a0ea532b3d66affa188d9e`. The standalone crate changes the inherited parser-floor underflow behavior so `popn` follows conventional stack semantics.

## License

Licensed under either of:

- Apache License, Version 2.0
- MIT License

at your option.
