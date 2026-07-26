# Changelog

All notable changes to this project are documented here. The project follows semantic versioning.

## [Unreleased]

- Preserve immutable unweighted stack DAGs during path-weight map and filter-map operations.
- Join weights directly when both operands already reference the same stack DAG, retaining the factored representation.
- Keep bounded top-first structural traversal inline for common stack depths.

## [0.2.0] - Unreleased

- Redesign the crate from first principles around weighted stack alternatives rather than exposing GLRMask's internal leveled representation.
- Add a small semantic API for construction, merging, stack operations, top partitioning, pop-then-push operations, and bounded canonical materialisation.
- Add an explicit path-local API for weight transformations, immutable weight iteration, and bounded structural traversal. Structural path layout is intentionally not part of the semantic contract.
- Add a mutable `VirtualStack` fast path for linear top prefixes over arbitrary hidden floors.
- Add exact stack-language interning for fixpoint visited sets.
- Validate the public API by compiling GLRMask solely through a compatibility adapter and passing its complete serial Rust library suite.
- Rebuild the Python 3.8+ ABI3 binding around the semantic API, with typed stubs and normal propagation of Python callback exceptions.
- Remove the Python requirement that weights be hashable.
- Keep `Weight` minimal: implementations provide ordinary equality and the associative, commutative, idempotent `join` operation.
- Remove representation identity and structural profiling from the standalone API; applications keep those concerns outside the data structure.
- Use `popn`, `StackOp`, `retain_where_at_depth`, and explicit structural-path method names.
- Keep synthetic merge-frontier nodes alive for the duration of pointer-keyed memoisation, preventing allocator address reuse from corrupting stack languages.
- Test the declared Rust 1.85 minimum in CI and avoid newer-only let-chain syntax.

## [0.1.0] - 2026-07-24

- Publish the initial standalone extraction of GLRMask's leveled GSS implementation.
- Add Rust and typed Python APIs, cross-platform wheels, and source distributions.

Version 0.2.0 replaces this extracted API with the from-scratch public abstraction described above.
