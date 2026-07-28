#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

cargo fmt --all --check
cargo test --all-targets --no-fail-fast
cargo test --doc
cargo clippy --all-targets -- -D warnings
cargo clippy --features python --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
cargo bench --no-run

if command -v cargo-fuzz >/dev/null 2>&1; then
    cargo +nightly fuzz build operation_sequences
else
    printf '%s\n' 'cargo-fuzz is not installed; skipped fuzz-target build.' >&2
fi
