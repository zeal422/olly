#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DOCS_DIR="$REPO_ROOT/docs"
ERRORS=0

EXPECTED_DOCS=(
    "architecture.md"
    "features.md"
    "workflow.md"
    "testing.md"
    "design-ui-ux.md"
    "refactoring.md"
    "release.md"
    "troubleshooting.md"
    "security.md"
)

echo "=== Documentation Health Check ==="
echo ""

# Check 1: All expected docs exist
echo "[1] Checking required docs files..."
for doc in "${EXPECTED_DOCS[@]}"; do
    path="$DOCS_DIR/$doc"
    if [ -f "$path" ]; then
        lines=$(wc -l < "$path")
        if [ "$lines" -lt 50 ]; then
            echo "  WARN: $doc exists but is short ($lines lines)"
        else
            echo "  OK: $doc ($lines lines)"
        fi
    else
        echo "  FAIL: $doc missing"
        ERRORS=$((ERRORS + 1))
    fi
done

# Check 2: Module line counts in AGENTS.md match actual
echo ""
echo "[2] Checking module line counts in AGENTS.md..."
checks=(
    "src/app/mod.rs:325"
    "src/app/state.rs:1061"
    "src/app/input_editor.rs:140"
    "src/app/chat_navigation.rs:63"
    "src/app/tui/mod.rs:765"
    "src/app/tui/chat.rs:338"
    "src/app/tui/input.rs:257"
    "src/app/tui/sidebar.rs:304"
    "src/app/tui/palette.rs:428"
    "src/app/tui/markdown.rs:301"
    "src/app/tui/modals.rs:227"
)

for check in "${checks[@]}"; do
    file="${check%%:*}"
    expected="${check##*:}"
    full_path="$REPO_ROOT/$file"
    if [ ! -f "$full_path" ]; then
        echo "  FAIL: $file not found"
        ERRORS=$((ERRORS + 1))
        continue
    fi
    actual=$(wc -l < "$full_path")
    diff=$((actual - expected))
    if [ "$diff" -lt 0 ]; then diff=$((-diff)); fi
    tolerance=10
    if [ "$diff" -gt "$tolerance" ]; then
        echo "  WARN: $file doc says ~$expected, actual $actual (diff: $diff)"
    else
        echo "  OK: $file ~$expected (actual: $actual)"
    fi
done

# Check 3: No broken internal doc links
echo ""
echo "[3] Checking for broken internal doc links..."
while IFS= read -r -d '' md; do
    while IFS= read -r link; do
        link_path="$REPO_ROOT/$link"
        if [ ! -f "$link_path" ]; then
            echo "  FAIL: Broken link in $(basename "$md"): $link"
            ERRORS=$((ERRORS + 1))
        fi
    done < <(grep -Eo '\([^)]*docs/[^)]+\)' "$md" 2>/dev/null | sed 's/^(docs\//docs\//' | sed 's/)$//' || true)
done < <(find "$REPO_ROOT" -name "*.md" -not -path "*/target/*" -print0)

if [ "$ERRORS" -eq 0 ]; then
    echo "  OK: No broken internal doc links found"
fi

# Summary
echo ""
if [ "$ERRORS" -gt 0 ]; then
    echo "Documentation check completed with $ERRORS issue(s)."
    exit 1
else
    echo "All documentation checks passed."
fi
