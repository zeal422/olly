#!/usr/bin/env bash
set -euo pipefail

FILTER="${1:-}"

echo "=== Running Unit Tests ==="
if [ -n "$FILTER" ]; then
    cargo test --lib -- "$FILTER"
else
    cargo test --lib
fi
echo ""
echo "All unit tests passed."
