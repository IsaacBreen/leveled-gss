# Validation and benchmarks — 2026-07-28

This record closes the standalone correctness-validation and benchmark work for `weighted-gss` 0.2.1.

## Revisions and environment

- implementation and benchmark commit: `4e1fdce8659a2a433db9292f3c206ac93bd29107`;
- merge commit on `main`: `2c6becb11919a29a8d38f8cf4e34a513131b39cc`;
- benchmark machine: Apple M1 Pro;
- operating system: macOS / Darwin 25.5.0, arm64;
- recorded toolchain: `rustc 1.91.0-nightly (54c581243 2025-08-25)`;
- benchmark command: `./scripts/run-benchmarks.sh -- --noplot`;
- source tree dirty state: clean.

The complete Criterion run exited successfully. Timings below are Criterion point estimates from that one machine and run. They are evidence about specific workload shapes, not portable absolute guarantees.

## Correctness evidence

The implementation is checked extensionally as a finite map from concrete stacks to joined weights.

The validation layers include:

- deterministic Rust tests for construction, merging, stack operations, weight transforms, bounded inspection, persistence, and deep structures;
- a shrinkable `proptest` state machine executed in lockstep with an explicit `BTreeMap<Vec<u8>, Bits>` model;
- deliberately colliding symbol hashes;
- bitwise-OR, minimum, and maximum join algebras;
- an oracle-backed `cargo-fuzz` target that checks semantic equivalence after every interpreted operation;
- checked-in fuzz seeds for construction, merging, empty stacks, transformations, and snapshot restoration;
- Rust 1.85 minimum-version checks;
- Rust and Python tests on Linux, macOS, and Windows;
- Python 3.8, 3.9, 3.12, and 3.14 checks;
- strict Clippy, rustdoc, package, wheel, source-distribution, Twine, and mypy checks.

A local non-ASan libFuzzer run completed 250,000 oracle-backed executions. AddressSanitizer on the local macOS host stalled before the target's `main` in ASan initialisation; the same target built and passed under the Linux GitHub Actions fuzz workflow. Both the pull-request workflows and the post-merge `main` workflows passed.

## Defects exposed by the suite

The new workloads found representation and algorithmic defects that ordinary unit tests had not exposed:

1. Batched constructors did not share common stack floors, making large shared languages unnecessarily expensive.
2. Merging separately constructed but equal single-stack floors rebuilt them symbol by symbol.
3. A 20,000-symbol common weighted prefix produced a one-node-per-symbol weighted chain and overflowed the process stack.
4. A 512-way half-overlap merge took roughly 5.6 ms before the shared-path fixes.
5. A 1,024-way join-heavy pop took roughly 14.7 ms before the shared constructor and compact weighted-segment fixes.

The fixes added a trie-based batched constructor, allocation-free exact equality for unweighted single paths, and a private compact weighted-segment node. Public semantics and API shape were preserved.

After the fixes, the two pathological operation measurements were approximately 134 µs for the 512-way half-overlap merge and 113 µs for the 1,024-way collapsing pop. The deep-prefix regressions now complete iteratively.

## Selected benchmark results

| Workload | `WeightedGss` median | Explicit-map median | Interpretation |
|---|---:|---:|---|
| Persistent fork of a 512-stack value | 174 ns | 141 µs | The persistent graph reuses the immutable source while the explicit map clones it |
| Merge two values sharing a 20,000-symbol top prefix | 8.25 µs | 22.6 µs | Segment and path reuse avoid copying the common prefix |
| Merge 512 half-overlapping alternatives | 134 µs | 58.9 µs | The explicit representation remains faster for this finite merge |
| Pop 1,024 alternatives that collapse to one stack | 113 µs | 42.6 µs | Joining a broad frontier still costs more than rebuilding the explicit result |
| Construct 1,024 weighted stacks in a binary language | 3.54 ms | 60.5 µs | Starting from already explicit stacks strongly favours the explicit map |
| Construct two weighted stacks with a 20,000-symbol common prefix | 968 µs | 6.96 µs | The graph pays substantial construction cost to build compact shared structure |

The unweighted suite similarly shows that graph operations can benefit from persistence and shared structure while construction and full materialisation often favour an explicit set.

## Interpretation

The benchmark conclusion is deliberately narrow:

- use an explicit map or set when the state is already concrete, construction dominates, or complete enumeration is routine;
- use `WeightedGss` when values persist across steps, fork frequently, share long structure, and undergo repeated incremental stack operations;
- do not reduce the suite to one headline speed ratio;
- do not impose absolute timing thresholds on shared GitHub-hosted runners.

The standalone suite measures public data-structure operations. The separate GLRMask/CFA adapter validation remains application-level evidence that the abstraction can support a demanding parser workload; it is not part of the standalone semantic specification.

## Reproduction

Run the semantic checks:

```bash
./scripts/run-validation.sh
```

Run the full Criterion suite and preserve environment metadata:

```bash
./scripts/run-benchmarks.sh -- --noplot
```

Run the coverage-guided oracle:

```bash
./scripts/run-fuzz.sh 300
```
