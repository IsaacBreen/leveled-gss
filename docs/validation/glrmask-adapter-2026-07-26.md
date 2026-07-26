# GLRMask adapter validation — 2026-07-26

This records the principal application-level acceptance test for the standalone `weighted-gss` rewrite: GLRMask implemented through the crate's documented public API, without retaining GLRMask's old graph representation.

## Revisions and environment

- `weighted-gss` candidate branch: `perf/slowall-final-20260726`, based on `819fd5c6a561ef2954fcf43ee1fa89cb8aa622af`.
- GLRMask adapter branch: `perf/weighted-gss-slowall-final-20260726`, based on `b44f2a85bff86d65811769b15504fe693a6792d8`.
- CFA benchmark revision: `8584f26f367575d9f467816690030694f7ac0320`.
- Machine: Apple M1 Pro MacBook Pro, 10 CPU cores, 16 GB RAM, macOS 26.5.
- GLRMask candidate wheel SHA-256: `6ab56f35a5347c88294ae1ea48feafd960c3db6dd1d124e6edf1aaadca966e12`.

The command for each leg was:

```sh
make --no-print-directory example-slow-all \
  FRAMEWORKS=glrmask_native \
  ARGS='--allow-single-framework' \
  OUTPUT=<leg>.json.zst \
  PYTHON=<clean-venv-python>
```

The suite used 50 measured runtime runs per example, no separate warm-up runs, and elementwise minimum reduction across runs. Build measurement used up to 20 runs with the CFA target-time policy. The full configuration is retained in each raw CFA artifact.

## Correctness and coverage

The comparison used an adapter → baseline → adapter bracket.

- Problems requested: **226**.
- Problems built in both implementations: **222**.
- Paired examples: **1,060**.
- Paired semantic steps: **611,642**.
- Semantic mismatches: **0**.
- Both adapter legs built **223** problems.
- The adapter built `jsb/data/Github_hard---o9882`; the adjacent baseline exceeded the 120-second build timeout.

Semantic comparison covered token counts, rejection positions, expected-token membership, and mask sizes at every paired step.

## Runtime result

The two adapter legs bracket the adjacent baseline. “Midpoint” is the elementwise average of the two adapter measurements before comparison with the baseline.

| Metric | Adapter before | Adapter after | Bracket midpoint | Mean midpoint delta |
|---|---:|---:|---:|---:|
| Mask time | +5.594% | +2.062% | **+3.828%** | +0.072 µs |
| Commit time | +2.099% | −5.460% | **−1.680%** | −0.026 µs |
| Total TBM | +4.052% | −1.422% | **+1.315%** | +0.046 µs |

For total TBM, the bracket-midpoint median per-step delta was **+0.085 µs**. Adapter total-TBM drift between the two legs was **−5.261%**, so the midpoint is more credible than either single adjacent comparison. This is a small but measurable runtime cost, not exact performance parity.

Selected build time over the 222 common builds was **−0.332%** at the bracket midpoint. Build measurements varied substantially between legs; the reliable conclusions are that no systematic build regression was established and that the adapter completed `o9882` when the baseline timed out.

## Final standalone-package checks

The candidate also passed:

- `cargo fmt --all --check`;
- `cargo test --all-targets` and doc tests;
- Clippy with warnings denied, with and without Python bindings;
- rustdoc with warnings denied;
- `cargo publish --dry-run`;
- abi3 Python wheel build for Python 3.8+;
- 10 Python binding tests;
- strict mypy consumer check;
- source-distribution build; and
- Twine metadata checks for both wheel and sdist.

The machine-readable bracket summary is in [`glrmask-adapter-2026-07-26.json`](glrmask-adapter-2026-07-26.json).
