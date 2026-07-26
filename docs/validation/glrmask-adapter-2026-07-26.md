# GLRMask implementation stress test — 2026-07-26

This record contains two related validation phases:

1. the historical broader pre-reduction candidate, used to prove that the underlying representation could support GLRMask; and
2. the final compact opt-in `engine` API, which preserves the useful performance primitives without exposing graph nodes or GLRMask-specific conveniences.

## Historical broader candidate

GLRMask was first implemented through weighted-gss while the crate still exposed parser-oriented traversal, virtual-stack, batched-operation, and stack-language-interning facilities.

### Revisions

- broader weighted-gss candidate: `59bebf9550d8b53c244f9e3b17d7bafacda1d343`
- GLRMask adapter: `de33574931371e940d242996292554b31e550e7a`
- CFA: `66415b0af3348ebd886021ac7627ce6ebe44f016`
- machine: Apple M1 Pro MacBook Pro, 10 CPU cores, 16 GB RAM, macOS 26.5

### Correctness and coverage

The exact broader candidate completed:

- **226** requested problems;
- **223** successful builds;
- **1,060** examples;
- **611,642** semantic steps;
- **0** semantic mismatches against the previous validated candidate;
- **0** coverage mismatches; and
- GLRMask's serial Rust library suite: **855 passed, 0 failed, 2 ignored**.

Semantic comparison covered token counts, rejection positions, expected-token membership, and mask sizes at every paired step.

### Performance bracket

The historical comparison used candidate → previous validated adapter → candidate with ten measured runs and elementwise-min reduction.

| Metric | Candidate before | Candidate after | Bracket midpoint | Median midpoint delta | p99 midpoint vs baseline |
|---|---:|---:|---:|---:|---:|
| Mask | -5.696% | -6.092% | **-5.894%** | -0.103 µs | 5.167 vs 5.625 µs |
| Commit | -3.611% | -2.591% | **-3.101%** | +0.020 µs | 5.104 vs 5.833 µs |
| Total TBM | -4.890% | -4.623% | **-4.756%** | -0.084 µs | 9.104 vs 10.208 µs |

The 99.9%-trimmed TBM result was **-4.664%**. This established that the representation was not inherently too slow for GLRMask. It did not establish the correct public abstraction boundary.

## Final compact engine follow-up

The final 0.2 design keeps the semantic weighted-stack API as the default surface and adds a small opt-in `engine` module. It contains only:

- representation-local path-weight iteration and filtering;
- bounded, coalescing concrete-stack visitation;
- a mutable linear top prefix over an unchanged hidden floor; and
- exact unweighted stack-language keys scoped to one interner.

Batched parser actions, guarded shifts, accumulator wrappers, representation IDs, graph nodes, and profiling summaries remain application-local.

### Final revisions

- weighted-gss compact engine code: `b541121c35a2e1b234fc2f7d8e9782368cfd0b6c`
- GLRMask compact adapter: `4158458e627a4301607075a8bb2429532f0257f7`
- CFA: `66415b0af3348ebd886021ac7627ce6ebe44f016`

The GLRMask branch is pinned to the immutable weighted-gss revision above with `features = ["engine"]`.

### Correctness

The compact adapter matched the exact broader adapter over:

- **226** requested problems;
- **1,060** examples;
- **611,642** semantic steps;
- **0** semantic mismatches; and
- **0** coverage mismatches.

GLRMask's internal parser fast-path equivalence and distributivity oracles were enabled across the full sweep. They completed on every instrumented problem except `Github_hard---o62060`, whose deliberately exhaustive oracle build hit the 120-second harness timeout. The normal compact build for that problem completed in **0.620 seconds** and matched all **8,011** recorded semantic steps exactly. The remaining oracle-enabled sweep covered **603,631** paired steps without an assertion failure.

The original failing schema, `Github_easy---o50163`, also passes with both parser oracles enabled.

A release-only adapter defect was found during this validation. Three calls placed the mutating `pop` operation inside `debug_assert_eq!`; optimized builds therefore compiled the mutation away. The fix performs the pop unconditionally and asserts only on its returned remainder. Four direct release-mode regression tests cover replace-top, single-target pop/push, multi-value pop/push, and a branch-root fallback.

Final GLRMask gates:

- release-mode adapter regressions: **4 passed**;
- full Rust library suite: **859 passed, 0 failed, 2 ignored**;
- remote-pinned CPython 3.12 wheel built successfully; and
- the original failing schema passed from that wheel.

### Performance bracket

The adjacent comparison used compact candidate → historical broad adapter → compact candidate, with ten measured runs on a fixed 224-problem cohort. `Github_hard---o62065` and `Github_hard---o21073` were excluded only from timing because of intermittent CFA worker deadlocks; both remain included in the semantic validation above.

Two independent build timeouts affected bracket coverage: the broad middle leg timed out on `Github_ultra---o21135`, and the first compact leg timed out on `Github_ultra---o62058`. The timing comparison therefore uses the **305,189** common semantic steps; there were **0** step-level mask or membership differences on those steps. Build comparison uses **219** common builds.

| Metric | Compact midpoint vs broad | Median delta | 99.9%-trimmed delta | p99 compact vs broad |
|---|---:|---:|---:|---:|
| Mask | **-15.548%** | -0.187 µs | -12.894% | 9.666 vs 12.125 µs |
| Commit | **-17.807%** | -0.147 µs | -15.566% | 11.669 vs 16.834 µs |
| Total TBM | **-16.725%** | -0.375 µs | -14.544% | 19.584 vs 25.834 µs |

Common-build time improved by **14.370%** at the bracket midpoint, with a median delta of **-28.355 ms**.

This result is stronger than the original stress-test requirement: the compact public engine boundary is not merely adequate for GLRMask; on this bracket it is materially faster than the broader adapter.

### weighted-gss release gates

The exact compact weighted-gss tree passed:

- default and `engine` Rust tests;
- default and `engine` doc tests;
- Clippy for default, `engine`, and Python feature checks with warnings denied;
- rustdoc with warnings denied;
- Rust 1.85 checks for default, `engine`, and Python features;
- `cargo publish --dry-run`;
- wheel and source-distribution builds;
- Twine validation;
- **11** clean wheel-install Python tests;
- strict mypy; and
- **11** clean source-distribution-install Python tests.

The machine-readable record is in [`glrmask-adapter-2026-07-26.json`](glrmask-adapter-2026-07-26.json).
