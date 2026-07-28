# Contributing

## Requirements

- Rust 1.85 or later
- Python 3.8 or later when working on bindings
- maturin 1.7 or later when working on packages

## Rust checks

```bash
cargo fmt --all --check
cargo test --all-targets
cargo test --doc
cargo clippy --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
cargo publish --dry-run
```

Python validation:

```bash
cargo clippy --features python --all-targets -- -D warnings
maturin build --release --out dist
python -m pip install --force-reinstall dist/weighted_gss-*.whl
python -m unittest discover -s python/tests -v
mypy --strict python/tests/typecheck_consumer.py
maturin sdist --out dist
python -m twine check dist/*
```

Run wheel and sdist installs in clean virtual environments before release.

## Design constraints

- Preserve stack-to-weight correlation.
- Treat `Weight::join` as associative, commutative, and idempotent.
- Keep representation details private.
- Document that `weights`, `map_weights`, and `filter_map_weights` operate on factored weight regions.
- Keep materialisation bounded; do not introduce it into hot stack operations.
- Keep representation-sensitive implementation details private.
- Preserve `LinearPrefix` behaviour over both complete and branched hidden floors.
- Test semantic operations against an explicit stack-to-weight model.
- Test any pointer-based memoisation against allocator address reuse.
- Validate substantial API changes through the GLRMask public-API adapter.

## Correctness and performance changes

Changes to stack semantics or graph algorithms should extend the explicit-model properties in `tests/properties.rs` and, when reachable from arbitrary operation sequences, the oracle-backed fuzz target. Performance-sensitive changes should be checked against an adjacent Criterion baseline on the same machine; see `docs/validation.md` and `docs/benchmarks.md`.

The full local validation command is:

```bash
./scripts/run-validation.sh
```
