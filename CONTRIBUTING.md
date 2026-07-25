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

When Python bindings are present, additionally run the wheel, sdist, type-stub, and cross-version checks documented in `docs/python.md`.

## Design constraints

- Preserve stack-to-weight correlation.
- Treat `Weight::join` as associative, commutative, and idempotent.
- Keep representation details private.
- Put path-local transformations behind `WeightedGss::paths()`.
- Keep materialisation bounded; do not introduce it into hot stack operations.
- Preserve the `VirtualStack` fast path for linear prefixes over both complete and branched floors.
- Test semantic operations against an explicit stack-to-weight model.
- Test any pointer-based memoisation against allocator address reuse.
- Validate substantial API changes through the GLRMask public-API adapter.
