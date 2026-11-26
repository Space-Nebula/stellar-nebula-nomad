#!/bin/bash
# Measure the local cost of the main CI stages so cache/build optimizations can
# be compared before and after a change. Usage:
#   ./scripts/ci-timing.sh clean /tmp/before.txt
#   ./scripts/ci-timing.sh warm  /tmp/after.txt
# The "clean" mode removes target/ first; "warm" reuses it.

set -euo pipefail

MODE="${1:-warm}"
OUTPUT="${2:-/tmp/ci-timing-$(date +%Y%m%d_%H%M%S).txt}"
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$PROJECT_ROOT"

if [ "$MODE" = "clean" ]; then
    echo "[ci-timing] Cleaning target/ for a cold measurement"
    rm -rf target
elif [ "$MODE" != "warm" ]; then
    echo "Usage: $0 {clean|warm} [output-file]" >&2
    exit 1
fi

: > "$OUTPUT"
record() {
    local label="$1"
    shift
    echo "[ci-timing] $label ..."
    local start end
    start=$(date +%s)
    "$@" >/dev/null 2>&1 || echo "[ci-timing] WARNING: $label failed" | tee -a "$OUTPUT"
    end=$(date +%s)
    printf '%-28s %6ss\n' "$label" "$((end - start))" | tee -a "$OUTPUT"
}

{
    echo "CI timing — mode=$MODE — $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "rustc: $(rustc --version 2>/dev/null || echo unavailable)"
    echo "cache: $([ -d target ] && echo warm || echo cold)"
    echo
} | tee -a "$OUTPUT"

record "cargo check --all-targets" cargo check --all-targets --locked
record "cargo build (native)"      cargo build --locked
record "cargo test --all"          cargo test --all --locked
record "cargo fmt --check"         cargo fmt --all -- --check
record "cargo clippy"              cargo clippy --all-targets --all-features --locked -- -D warnings
record "cargo doc"                 env RUSTDOCFLAGS=-Dmissing_docs cargo doc --all --no-deps --locked

echo "[ci-timing] Results written to $OUTPUT"
