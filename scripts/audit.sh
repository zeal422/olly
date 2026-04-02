#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo-audit &> /dev/null; then
    echo "cargo-audit is required for dependency vulnerability scanning."
    echo "Install: cargo install cargo-audit"
    exit 1
fi

echo "=== Dependency Audit ==="
if ! cargo audit; then
    echo ""
    echo "WARNING: Vulnerable dependencies detected."
    echo "Review the report above and update affected crates."
    exit 1
fi

echo "No known vulnerabilities found."
