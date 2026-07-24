# leveled-gss

A persistent, weighted graph-structured stack with leveled sharing, written in Rust.

The crate represents many stacks in one immutable graph. Common suffixes are shared, branching and merging preserve path-specific accumulators, and deterministic prefixes can be manipulated through a mutable `VirtualStack` fast path. The implementation was extracted from the data structure used by [GLRMask](https://github.com/IsaacBreen/glrmask).

The API is currently experimental. This repository starts by preserving the production implementation and its regression tests; API reduction and broader documentation can follow without forcing a second, divergent implementation.

## Example

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

## Main types

- `LeveledGSS<T, A>`: persistent shared set of stacks.
- `VirtualStack<T, A>`: mutable view of a deterministic stack prefix.
- `Merge`: accumulator combination at convergent paths.
- `LeveledGSSSummary`: structural diagnostics without enumerating all paths.

`LeveledGSS::to_stacks` materializes concrete stacks only up to an explicit limit. It is useful for tests and inspection, but it is deliberately not the main execution model.

## Provenance

The initial standalone extraction tracks `glrmask` commit `58c24ff44e3a796172a0ea532b3d66affa188d9e`. The implementation includes the production regression suite from `src/ds/leveled_gss.rs` and its private stack-segment backends.

## License

Licensed under either of:

- Apache License, Version 2.0
- MIT License

at your option.
