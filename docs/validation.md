# Correctness validation

`WeightedGss<S, W>` has an extensional meaning: a finite map from concrete stacks to joined weights. Validation therefore compares the compact implementation against deliberately simple explicit representations rather than against its private graph structure.

## Deterministic tests

The ordinary integration tests cover:

- construction, merge, push, pop, `popn`, and top selection;
- weight joining when different paths collapse to the same concrete stack;
- factored weight mapping and filtering;
- bounded visitors and materialisation;
- deep deterministic stacks;
- large shared stack languages;
- persistence and allocator-reuse regressions;
- the Rust and Python public APIs.

Run them with:

```bash
cargo test --all-targets
cargo test --doc
```

## Property-based state-machine test

`tests/properties.rs` generates shrinkable operation sequences and executes them against two states in lockstep:

1. the real `WeightedGss<u8, Bits>`;
2. an explicit `BTreeMap<Vec<u8>, Bits>` reference model.

The generated operations include:

- adding and merging alternatives;
- push, pop, `popn`, `retain_top`, `retain_empty`, and `pop_top`;
- join-preserving `map_weights` and `filter_map_weights` transformations;
- restoring earlier persistent snapshots.

After every operation it checks:

- complete canonical materialisation;
- `is_empty`, `max_depth`, `top`, `tops`, and `has_empty_stack`;
- `joined_weight`;
- `for_each_stack_top_first` output;
- atomic failure when a visitor or materialiser limit is too small;
- `LinearPrefix` round trips and mutations whenever a prefix is available;
- preservation of the previous immutable snapshot.

The suite also tests deliberately colliding symbol hashes and several valid join algebras. A failing random case is automatically reduced to a smaller operation sequence by `proptest`.

Run only this layer with:

```bash
cargo test --test properties
```

## Coverage-guided fuzzing

`fuzz/fuzz_targets/operation_sequences.rs` interprets arbitrary bytes as bounded operation sequences. It uses the same extensional oracle strategy and checks the real implementation after every step. This is an oracle-backed target: semantic disagreement fails even when neither implementation panics.

The checked-in seed corpus exercises construction, merging, empty stacks, transformations, and snapshot restoration.

```bash
cargo install cargo-fuzz --locked
cargo +nightly fuzz run operation_sequences
```

The `Fuzz` GitHub Actions workflow:

- builds the target and runs a short smoke fuzz on pushes and pull requests;
- runs a longer job every Monday;
- accepts a custom duration through `workflow_dispatch`;
- uploads generated failure artifacts.

## Scope

These layers validate the supported standalone abstraction. The historical GLRMask adapter and CFA corpus remain useful application-level stress evidence, but they are not the specification and are intentionally not required to test this repository.

## Completed validation campaign

The validation and benchmark work completed on 2026-07-28 is recorded in [Validation and benchmarks — 2026-07-28](https://github.com/IsaacBreen/weighted-gss/blob/main/docs/validation/validation-and-benchmarks-2026-07-28.md). The record identifies the tested revisions, environment, checks, selected measurements, defects found, and interpretation limits.
