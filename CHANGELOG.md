# Changelog

All notable changes to this project are documented here. The project follows semantic versioning.

## [Unreleased]

## [0.2.0] - Unreleased

- Redesign the crate from first principles around weighted stack alternatives rather than exposing GLRMask's internal leveled representation.
- Add a small semantic API for construction, merging, stack operations, top partitioning, stack effects, and bounded canonical materialisation.
- Add an explicit path-local API for weight transformations and structural traversal.
- Add a mutable `VirtualStack` fast path for linear top prefixes over arbitrary hidden floors.
- Add exact stack-language interning for fixpoint visited sets.
- Validate the public API by compiling GLRMask solely through a compatibility adapter and passing its complete serial Rust library suite.
- Rebuild the Python 3.8+ ABI3 binding around the semantic API, with typed stubs and normal propagation of Python callback exceptions.
- Remove the Python requirement that weights be hashable.
- Keep synthetic merge-frontier nodes alive for the duration of pointer-keyed memoisation, preventing allocator address reuse from corrupting stack languages.
- Test the declared Rust 1.85 minimum in CI and avoid newer-only let-chain syntax.
- Replace address-derived public representation keys with lazily assigned, process-unique IDs that cannot collide after allocator reuse.

## [0.1.0] - 2026-07-24

- Publish the initial standalone extraction of GLRMask's leveled GSS implementation.
- Add Rust and typed Python APIs, cross-platform wheels, and source distributions.

Version 0.2.0 replaces this extracted API with the from-scratch public abstraction described above.
