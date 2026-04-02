#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC_DIR="$REPO_ROOT/src"

echo "=== Lines of Code by Module ==="
echo ""
printf "%-50s %8s\n" "File" "Lines"
printf "%-50s %8s\n" "$(printf '%0.s-' {1..50})" "$(printf '%0.s-' {1..8})"

total=0
declare -A groups

while IFS= read -r -d '' file; do
    relative="${file#$REPO_ROOT/}"
    lines=$(wc -l < "$file")
    total=$((total + lines))

    # Group by top-level module
    group=$(echo "$relative" | cut -d'/' -f1-2)
    groups["$group"]=$(( ${groups["$group"]:-0} + lines ))

    printf "%-50s %8d\n" "$relative" "$lines"
done < <(find "$SRC_DIR" -name "*.rs" -print0 | sort -z)

printf "%-50s %8s\n" "$(printf '%0.s-' {1..50})" "$(printf '%0.s-' {1..8})"
printf "%-50s %8d\n" "TOTAL" "$total"
echo ""
echo "=== By Module Group ==="
echo ""
printf "%-30s %8s\n" "Module" "Lines"
printf "%-30s %8s\n" "$(printf '%0.s-' {1..30})" "$(printf '%0.s-' {1..8})"

for key in "${!groups[@]}"; do
    printf "%s\t%d\n" "$key" "${groups[$key]}"
done | sort -t$'\t' -k2 -rn
