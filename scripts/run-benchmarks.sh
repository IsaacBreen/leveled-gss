#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
out_dir="target/benchmark-runs/$timestamp"
mkdir -p "$out_dir"

{
    printf 'timestamp_utc=%s\n' "$timestamp"
    printf 'commit=%s\n' "$(git rev-parse HEAD)"
    printf 'branch=%s\n' "$(git branch --show-current)"
    printf 'dirty=%s\n' "$(test -n "$(git status --porcelain)" && echo true || echo false)"
    printf 'rustc=%s\n' "$(rustc --version --verbose | tr '\n' ';')"
    printf 'cargo=%s\n' "$(cargo --version)"
    printf 'uname=%s\n' "$(uname -a)"
    if command -v sysctl >/dev/null 2>&1; then
        sysctl -n machdep.cpu.brand_string 2>/dev/null | sed 's/^/cpu=/' || true
    fi
    if command -v lscpu >/dev/null 2>&1; then
        lscpu | sed 's/^/lscpu: /'
    fi
} > "$out_dir/environment.txt"

printf 'Benchmark metadata: %s\n' "$out_dir/environment.txt"
set -o pipefail
cargo bench --bench construction --bench operations --bench materialization --bench unweighted "$@" 2>&1 | tee "$out_dir/output.txt"
