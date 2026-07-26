# Changelog

All notable changes to this project are documented here. The project follows semantic versioning.

## [0.2.0] - Unreleased

- Redesign the crate around a small semantic abstraction: weighted stack alternatives, merging, ordinary stack operations, top selection, and bounded canonical materialisation.
- Keep the graph representation and all parser-engine machinery private.
- Export only `Weight`, `WeightedGss`, the unweighted `Gss` alias, and `PathLimitExceeded`.
- Add persistent constructors for independently weighted stacks and homogeneous alternatives that share one weight.
- Use ordinary equality to factor equal weights over shared stack languages.
- Preserve immutable stack DAG sharing and share terminal stack suffixes across constructed alternatives.
- Rebuild the Python 3.8+ ABI3 binding around semantic stack operations, typed stubs, unhashable weights, and normal propagation of Python callback exceptions.
- Remove representation identity, structural profiling, path traversal, virtual stacks, batched stack operations, and stack-language interning from the supported Rust API.
- Validate core semantics against an explicit stack-to-weight map and test the declared Rust 1.85 minimum in CI.
- Retain the previous GLRMask adapter as historical implementation and performance evidence, not as the specification of the public crate surface.

## [0.1.0] - 2026-07-24

- Publish the initial standalone extraction of GLRMask's leveled GSS implementation.
- Add Rust and typed Python APIs, cross-platform wheels, and source distributions.

Version 0.2.0 replaces the extracted API with the smaller abstraction described above.
