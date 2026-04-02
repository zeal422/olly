#!/usr/bin/env bash
set -euo pipefail

HTML=""
OUTPUT_DIR="target/coverage"

for arg in "$@"; do
    case "$arg" in
        --html) HTML="1" ;;
        --output-dir) shift; OUTPUT_DIR="$1" ;;
    esac
done

if ! command -v cargo-llvm-cov &> /dev/null; then
    echo "cargo-llvm-cov is required for coverage reporting."
    echo "Install: cargo install cargo-llvm-cov"
    echo "Requires: LLVM toolchain (rustup component add llvm-tools-preview)"
    exit 1
fi

echo "=== Test Coverage Report ==="
echo ""

mkdir -p "$OUTPUT_DIR"

if [ -n "$HTML" ]; then
    echo "Generating HTML coverage report..."
    if ! cargo llvm-cov --all-targets --all-features --html --output-dir "$OUTPUT_DIR"; then
        echo "ERROR: Coverage report generation failed."
        exit 1
    fi
    echo ""
    echo "HTML report: $OUTPUT_DIR/index.html"
else
    echo "Generating text coverage report..."
    if ! cargo llvm-cov --all-targets --all-features; then
        echo "ERROR: Coverage report generation failed."
        exit 1
    fi
fi

echo ""
if [ -n "$HTML" ]; then
    echo "Coverage report generated in $OUTPUT_DIR"
else
    echo "Coverage report written to stdout"
fi
