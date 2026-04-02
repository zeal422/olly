#!/usr/bin/env bash
set -euo pipefail

YES=""
for arg in "$@"; do
    case "$arg" in
        -y|--yes) YES="1" ;;
    esac
done

if [ -z "$YES" ]; then
    echo "This will remove all build artifacts from target/."
    read -rp "Proceed? [y/N] " response
    if [[ "$response" != [yY] ]]; then
        echo "Cancelled."
        exit 0
    fi
fi

echo "=== Cleaning Build Artifacts ==="
cargo clean
echo "Done. All build artifacts removed."
