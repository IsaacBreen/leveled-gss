# GLRMask adapter validation — 2026-07-26

This is the application-level acceptance test for the standalone `weighted-gss` 0.2 rewrite. GLRMask was implemented through the crate's documented Rust API; it did not retain or access the old graph representation.

## Final revisions

- weighted-gss: `59bebf9550d8b53c244f9e3b17d7bafacda1d343` (`rewrite/from-scratch-20260725`)
- GLRMask adapter: `de33574931371e940d242996292554b31e550e7a` (`feature/weighted-gss-adapter-20260725`)
- CFA: `66415b0af3348ebd886021ac7627ce6ebe44f016`
- Machine: Apple M1 Pro MacBook Pro, 10 CPU cores, 16 GB RAM, macOS 26.5
- Final adapter wheel SHA-256: `ccb7dfd2e15c9ffaaeb8078d15bc78fc19a81d9cb21e04a2606cd66b6dc41372`

The sweep command was:

```sh
make --no-print-directory example-slow-all \
  FRAMEWORKS=glrmask_native \
  ARGS='--allow-single-framework' \
  OUTPUT=<artifact>.json.zst \
  PYTHON=<clean-venv-python> \
  TIMING_RUNS=10 BUILD_RUNS=1
```

There were no warm-up runs. Runtime values below use CFA's elementwise minimum across the ten measured runs. Recorded per-step maxima were inspected separately and were substantially noisier because occasional millisecond interruptions moved between unrelated steps and problems.

## Correctness and coverage

The exact final tree completed:

- **226** requested problems;
- **223** successful builds;
- **1,060** examples;
- **611,642** semantic steps;
- **0** semantic mismatches against the previous validated candidate;
- **0** coverage mismatches; and
- GLRMask's complete serial library suite: **855 passed, 0 failed, 2 ignored**.

Semantic comparison covered token counts, rejection positions, expected-token membership, and mask sizes at every paired step.

## Clean-API performance bracket

The principal comparison used final clean-API adapter → previous validated adapter → final clean-API adapter. The weighted-gss candidate for this bracket was `d0c76ad`; the later `59bebf9` tree adds private diagnostics and terminal-suffix sharing without changing the public runtime contract.

One build (`jsb/data/Github_hard---o21073`) timed out only in the first candidate leg. The bracket therefore compares the **222** builds and **306,361** mask steps common to all three legs. There were **0 semantic mismatches** on the common coverage.

| Metric | Candidate before | Candidate after | Bracket midpoint | Median midpoint delta | p99 midpoint vs baseline |
|---|---:|---:|---:|---:|---:|
| Mask | -5.696% | -6.092% | **-5.894%** | -0.103 µs | 5.167 vs 5.625 µs |
| Commit | -3.611% | -2.591% | **-3.101%** | +0.020 µs | 5.104 vs 5.833 µs |
| Total TBM | -4.890% | -4.623% | **-4.756%** | -0.084 µs | 9.104 vs 10.208 µs |

The 99.9%-trimmed TBM result was **-4.664%**. This establishes that the cleaned 0.2 API did not require a runtime compromise for GLRMask's workload.

Build timings were more order-sensitive. Across the 222 common builds, total selected build time was +5.006% at the bracket midpoint, while median build time was lower for the candidate (169.1 ms versus 191.2 ms). No meaningful build regression was established.

## Exact final-tree confirmation

After porting terminal-suffix sharing and the private Python structure dump, the exact `59bebf9` tree was run again. It matched the previous candidate on all 226 problems and all 611,642 semantic steps.

Observed elementwise-min timings in that adjacent run were:

| Metric | Total delta | Median delta | p99 final vs baseline |
|---|---:|---:|---:|
| Mask | -8.870% | -0.125 µs | 4.958 vs 5.625 µs |
| Commit | -7.259% | +0.000 µs | 4.709 vs 5.833 µs |
| Total TBM | -8.202% | -0.166 µs | 8.749 vs 10.208 µs |

Because this was a single adjacent leg rather than a bracket, its magnitude is confirmation rather than the primary comparative estimate. It showed no regression after the final two ports.

## Release-package gates

The exact final tree passed:

- `cargo fmt --all --check`;
- all Rust targets and doc tests;
- Clippy with warnings denied, with and without Python bindings;
- rustdoc with warnings denied;
- Rust 1.85 checks, with and without Python bindings;
- `cargo publish --dry-run`;
- ABI3 wheel and source-distribution builds;
- Twine validation for wheel and sdist;
- clean wheel install and **11** Python tests;
- clean sdist install and **11** Python tests; and
- strict mypy consumer checking.

The machine-readable record is in [`glrmask-adapter-2026-07-26.json`](glrmask-adapter-2026-07-26.json).
