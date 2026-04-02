#!/usr/bin/env bash
set -euo pipefail

MODEL=""
URL=""
RELEASE=""

for arg in "$@"; do
    case "$arg" in
        --model) shift; MODEL="$1" ;;
        --url) shift; URL="$1" ;;
        --release) RELEASE="1" ;;
    esac
done

DEFAULT_MODEL="llama3.2:latest"
DEFAULT_URL="http://localhost:11434"

MODEL="${MODEL:-$DEFAULT_MODEL}"
URL="${URL:-$DEFAULT_URL}"

echo "=== Running Olly (dev) ==="
echo "Model: $MODEL"
echo "URL: $URL"
echo ""

BUILD_ARGS=()
if [ -n "$RELEASE" ]; then
    BUILD_ARGS+=(--release)
fi
BUILD_ARGS+=(-- --url "$URL" --model "$MODEL")

cargo run "${BUILD_ARGS[@]}"
