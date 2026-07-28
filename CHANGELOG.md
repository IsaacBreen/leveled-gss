# Changelog

All notable changes to this project are documented here. The project follows semantic versioning.

## [0.2.1] - 2026-07-28

- Remove the `engine` feature and `weighted_gss::engine` module.
- Remove `StackLanguageInterner` and `StackLanguageId`; canonical fixpoint-key machinery is application-specific rather than part of the stack abstraction.
- Export `LinearPrefix`, `linear_prefix`, and `for_each_stack_top_first` directly from the crate root.
- Replace representation-sensitive `PathLimitExceeded` with one opaque `StackLimitExceeded` shared by bounded visitation and materialisation.
- Make `to_stacks(max_stacks)` bound distinct concrete output stacks rather than internal encoded paths.
- Rename the Python `to_stacks` keyword from `max_paths` to `max_stacks`.
- Add deterministic, property-based, and coverage-guided oracle validation against explicit stack-to-weight maps.
- Add Criterion benchmarks against explicit-map, weight-partitioned, and unweighted-set baselines.
- Share common stack floors during batched construction and reuse exactly equal independently constructed single paths during merge.
- Add compact weighted segments for long common prefixes, eliminating deep weighted-stack overflow and making deep-prefix operations iterative.
- Add scheduled and pull-request fuzzing plus expanded benchmark and validation documentation.

## [0.2.0] - 2026-07-27

- Redesign the crate around a small semantic abstraction: weighted stack alternatives, merging, ordinary stack operations, top selection, and bounded canonical materialisation.
- Keep the graph representation private and the default API independent of parser-engine machinery.
- Export only `Weight`, `WeightedGss`, the unweighted `Gss` alias, and `PathLimitExceeded` by default.
- Add persistent constructors for independently weighted stacks and homogeneous alternatives that share one weight.
- Use ordinary equality to factor equal weights over shared stack languages.
- Preserve immutable stack DAG sharing and share terminal stack suffixes across constructed alternatives.
- Rebuild the Python 3.8+ ABI3 binding around semantic stack operations, typed stubs, unhashable weights, and normal propagation of Python callback exceptions.
- Add core `weights`, `map_weights`, and `filter_map_weights` operations over documented factored weight regions.
- Add an opt-in `engine` module containing only bounded concrete-stack inspection, a linear-prefix view, and exact stack-language IDs.
- Make stack-language interning iterative for deep graphs and canonical unions while preserving the recursive fast path for shallow frontiers.
- Remove representation identity, structural profiling, raw graph traversal, batched stack operations, and parser-specific conveniences from the supported Rust API.
- Validate core semantics against an explicit stack-to-weight map and test the declared Rust 1.85 minimum in CI.
- Validate the opt-in engine surface through a GLRMask adapter while keeping GLRMask-specific conveniences in GLRMask.

## [0.1.0] - 2026-07-24

- Publish the initial standalone extraction of GLRMask's leveled GSS implementation.
- Add Rust and typed Python APIs, cross-platform wheels, and source distributions.

Version 0.2.0 replaces the extracted API with the smaller abstraction described above.
