#!/usr/bin/env bash
set -euo pipefail

FILTER="${1:-}"

echo "=== Running Integration Tests ==="
if [ -n "$FILTER" ]; then
    cargo test --test '*' -- "$FILTER"
else
    cargo test --test '*'
fi
echo ""
echo "All integration tests passed."
