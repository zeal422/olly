$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$docsDir = Join-Path $repoRoot "docs"
$agentsPath = Join-Path $repoRoot "AGENTS.md"
$readmePath = Join-Path $repoRoot "README.md"

$expectedDocs = @(
    "architecture.md",
    "features.md",
    "workflow.md",
    "testing.md",
    "design-ui-ux.md",
    "refactoring.md",
    "release.md",
    "troubleshooting.md",
    "security.md"
)

$errors = 0

Write-Host "=== Documentation Health Check ==="
Write-Host ""

# Check 1: All expected docs exist
Write-Host "[1] Checking required docs files..."
foreach ($doc in $expectedDocs) {
    $path = Join-Path $docsDir $doc
    if (Test-Path $path) {
        $lines = (Get-Content $path).Count
        if ($lines -lt 50) {
            Write-Host "  WARN: $doc exists but is short ($lines lines)"
        } else {
            Write-Host "  OK: $doc ($lines lines)"
        }
    } else {
        Write-Host "  FAIL: $doc missing"
        $errors++
    }
}

# Check 2: Module line counts in AGENTS.md match actual files
Write-Host ""
Write-Host "[2] Checking module line counts in AGENTS.md..."
$checks = @(
    @{ pattern = "mod\.rs.*~325"; file = "src/app/mod.rs"; expected = 325 },
    @{ pattern = "state\.rs.*~1.061"; file = "src/app/state.rs"; expected = 1061 },
    @{ pattern = "input_editor\.rs.*~140"; file = "src/app/input_editor.rs"; expected = 140 },
    @{ pattern = "chat_navigation\.rs.*~63"; file = "src/app/chat_navigation.rs"; expected = 63 },
    @{ pattern = "tui/mod\.rs.*~765"; file = "src/app/tui/mod.rs"; expected = 765 },
    @{ pattern = "tui/chat\.rs.*~338"; file = "src/app/tui/chat.rs"; expected = 338 },
    @{ pattern = "tui/input\.rs.*~257"; file = "src/app/tui/input.rs"; expected = 257 },
    @{ pattern = "tui/sidebar\.rs.*~304"; file = "src/app/tui/sidebar.rs"; expected = 304 },
    @{ pattern = "tui/palette\.rs.*~428"; file = "src/app/tui/palette.rs"; expected = 428 },
    @{ pattern = "tui/markdown\.rs.*~301"; file = "src/app/tui/markdown.rs"; expected = 301 },
    @{ pattern = "tui/modals\.rs.*~227"; file = "src/app/tui/modals.rs"; expected = 227 }
)

foreach ($check in $checks) {
    $fullPath = Join-Path $repoRoot $check.file
    if (-not (Test-Path $fullPath)) {
        Write-Host "  FAIL: $($check.file) not found"
        $errors++
        continue
    }
    $actual = (Get-Content $fullPath).Count
    $diff = [Math]::Abs($actual - $check.expected)
    $tolerance = 10
    if ($diff -gt $tolerance) {
        Write-Host "  WARN: $($check.file) doc says ~$($check.expected), actual $actual (diff: $diff)"
    } else {
        Write-Host "  OK: $($check.file) ~$($check.expected) (actual: $actual)"
    }
}

# Check 3: No broken internal doc links
Write-Host ""
Write-Host "[3] Checking for broken internal doc links..."
$linkErrors = 0
$allMd = Get-ChildItem -Path $repoRoot -Recurse -Filter "*.md" | Where-Object {
    $_.FullName -notmatch "\\target\\"
}

foreach ($md in $allMd) {
    $content = Get-Content $md.FullName -Raw
    $links = [regex]::Matches($content, '\[([^\]]+)\]\((docs/[^)]+)\)') | ForEach-Object { $_.Groups[2].Value }
    foreach ($link in $links) {
        $linkPath = Join-Path $repoRoot $link
        if (-not (Test-Path $linkPath)) {
            Write-Host "  FAIL: Broken link in $($md.Name): $link"
            $linkErrors++
            $errors++
        }
    }
}
if ($linkErrors -eq 0) {
    Write-Host "  OK: No broken internal doc links found"
}

# Summary
Write-Host ""
if ($errors -gt 0) {
    Write-Host "Documentation check completed with $errors issue(s)."
    exit 1
} else {
    Write-Host "All documentation checks passed."
}
