#!/usr/bin/env bash
set -euo pipefail

FILTER="${1:-}"

echo "=== Running Integration Tests ==="
if [ -n "$FILTER" ]; then
    cargo test --tests -- "$FILTER"
else
    cargo test --tests
fi
echo ""
echo "All integration tests passed."
