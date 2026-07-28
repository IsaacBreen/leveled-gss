#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

if ! command -v cargo-fuzz >/dev/null 2>&1; then
    printf '%s\n' 'Install cargo-fuzz first: cargo install cargo-fuzz --locked' >&2
    exit 1
fi

exec cargo +nightly fuzz run operation_sequences "$@"
