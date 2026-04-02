param(
    [switch]$Html,
    [string]$OutputDir = "target/coverage"
)

$ErrorActionPreference = "Stop"

if (-not (Get-Command cargo-llvm-cov -ErrorAction SilentlyContinue)) {
    Write-Host "cargo-llvm-cov is required for coverage reporting."
    Write-Host "Install: cargo install cargo-llvm-cov"
    Write-Host "Requires: LLVM toolchain (rustup component add llvm-tools-preview)"
    exit 1
}

Write-Host "=== Test Coverage Report ==="
Write-Host ""

$reportDir = Join-Path $PSScriptRoot "..\$OutputDir"
New-Item -ItemType Directory -Path $reportDir -Force | Out-Null

if ($Html) {
    Write-Host "Generating HTML coverage report..."
    cargo llvm-cov --all-targets --all-features --html --output-dir $reportDir
    if ($LASTEXITCODE -ne 0) {
        Write-Host "ERROR: Coverage report generation failed."
        exit $LASTEXITCODE
    }
    $indexPath = Join-Path $reportDir "index.html"
    Write-Host ""
    Write-Host "HTML report: $indexPath"
} else {
    Write-Host "Generating text coverage report..."
    cargo llvm-cov --all-targets --all-features
    if ($LASTEXITCODE -ne 0) {
        Write-Host "ERROR: Coverage report generation failed."
        exit $LASTEXITCODE
    }
}

Write-Host ""
Write-Host "Coverage report generated in $OutputDir"
