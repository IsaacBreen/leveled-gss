# Contributing

## Requirements

- Rust 1.85 or later
- Python 3.8 or later for the bindings
- maturin 1.7 or later

## Checks

Run the complete local validation:

```bash
cargo fmt --all --check
STACKVEC=vec cargo test --all-targets
STACKVEC=arc cargo test --all-targets
cargo test --doc
cargo clippy --all-targets -- -D warnings
cargo clippy --features python --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
cargo publish --dry-run
maturin build --release --out dist
python -m pip install --force-reinstall dist/leveled_gss-*.whl
python -m unittest discover -s python/tests -v
```

Public Rust API additions must include rustdoc because the crate denies `missing_docs`. Python API additions must update runtime docstrings, `python/leveled_gss/__init__.pyi`, and Python tests.

## Design constraints

- Preserve path-to-accumulator correlation.
- Treat `Merge` as an associative, commutative, idempotent join.
- Keep `to_stacks` bounded; do not introduce unbounded materialization into hot paths.
- Run semantic tests against both `STACKVEC` backends.
