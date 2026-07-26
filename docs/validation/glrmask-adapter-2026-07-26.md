# Historical GLRMask implementation stress test — 2026-07-26

This record concerns the **broader pre-reduction candidate**, not the final supported Rust API for weighted-gss 0.2.

GLRMask was temporarily implemented through weighted-gss while the crate still exposed parser-oriented traversal, virtual-stack, batched-operation, and stack-language-interning facilities. That exercise established that the underlying representation had the required semantics and performance. Those facilities were subsequently removed from the public API because GLRMask was a stress test, not the intended abstraction boundary.

## Revisions

- broader weighted-gss candidate: `59bebf9550d8b53c244f9e3b17d7bafacda1d343`
- GLRMask adapter: `de33574931371e940d242996292554b31e550e7a`
- CFA: `66415b0af3348ebd886021ac7627ce6ebe44f016`
- machine: Apple M1 Pro MacBook Pro, 10 CPU cores, 16 GB RAM, macOS 26.5

The adapter branch should remain pinned to the broader candidate. It is historical validation and is not expected to compile against the reduced 0.2 public surface.

## Correctness and coverage

The exact broader candidate completed:

- **226** requested problems;
- **223** successful builds;
- **1,060** examples;
- **611,642** semantic steps;
- **0** semantic mismatches against the previous validated candidate;
- **0** coverage mismatches; and
- GLRMask's serial Rust library suite: **855 passed, 0 failed, 2 ignored**.

Semantic comparison covered token counts, rejection positions, expected-token membership, and mask sizes at every paired step.

## Performance bracket

The principal comparison used candidate → previous validated adapter → candidate with ten measured runs and elementwise-min reduction.

| Metric | Candidate before | Candidate after | Bracket midpoint | Median midpoint delta | p99 midpoint vs baseline |
|---|---:|---:|---:|---:|---:|
| Mask | -5.696% | -6.092% | **-5.894%** | -0.103 µs | 5.167 vs 5.625 µs |
| Commit | -3.611% | -2.591% | **-3.101%** | +0.020 µs | 5.104 vs 5.833 µs |
| Total TBM | -4.890% | -4.623% | **-4.756%** | -0.084 µs | 9.104 vs 10.208 µs |

The 99.9%-trimmed TBM result was **-4.664%**. The point of this result is narrow: the representation was not inherently too slow for GLRMask. It does not justify exporting GLRMask's supporting mechanisms from the general-purpose crate.

## Interpretation for 0.2

The final 0.2 API deliberately retains only weighted stack semantics. This stress test remains useful evidence for implementation quality, but it must not be cited as describing the current public surface.

The machine-readable historical record remains in [`glrmask-adapter-2026-07-26.json`](glrmask-adapter-2026-07-26.json).
