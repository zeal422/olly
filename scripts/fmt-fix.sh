#!/usr/bin/env bash
set -euo pipefail

ALLOW_DIRTY=""
NO_CLIPPY=""

for arg in "$@"; do
    case "$arg" in
        --allow-dirty) ALLOW_DIRTY="--allow-dirty --allow-staged" ;;
        --no-clippy) NO_CLIPPY="1" ;;
    esac
done

echo "=== Formatting Fix ==="
echo "Applying cargo fmt..."
cargo fmt --all
echo "Formatting applied."

if [ -z "$NO_CLIPPY" ]; then
    echo ""
    echo "=== Clippy Auto-Fix ==="
    if ! cargo clippy --all-targets --all-features --fix $ALLOW_DIRTY -- -D warnings; then
        echo ""
        echo "WARNING: clippy --fix could not resolve all issues."
        echo "Run './scripts/check.sh' to see remaining problems."
        exit 1
    fi
    echo "Clippy fixes applied."
fi

echo ""
echo "Done. Run './scripts/check.sh' to verify."
