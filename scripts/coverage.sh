#!/usr/bin/env bash
set -euo pipefail

HTML=""
OUTPUT_DIR="target/coverage"

while [ "$#" -gt 0 ]; do
    case "$1" in
        --html) HTML="1"; shift ;;
        --output-dir)
            if [ $# -lt 2 ]; then
                echo "Error: --output-dir requires a value"
                echo "Usage: $0 [--html] [--output-dir <path>]"
                exit 1
            fi
            OUTPUT_DIR="$2"; shift 2 ;;
        *) shift ;;
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
