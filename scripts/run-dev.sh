#!/usr/bin/env bash
set -euo pipefail

MODEL=""
URL=""
RELEASE=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --model)
            if [ $# -lt 2 ]; then
                echo "Error: --model requires a value"
                echo "Usage: $0 [--model <name>] [--url <url>] [--release]"
                exit 1
            fi
            MODEL="$2"; shift 2 ;;
        --url)
            if [ $# -lt 2 ]; then
                echo "Error: --url requires a value"
                echo "Usage: $0 [--model <name>] [--url <url>] [--release]"
                exit 1
            fi
            URL="$2"; shift 2 ;;
        --release) RELEASE="1"; shift ;;
        *) shift ;;
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
