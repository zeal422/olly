$ErrorActionPreference = "Stop"

if (-not (Get-Command cargo-audit -ErrorAction SilentlyContinue)) {
    Write-Host "cargo-audit is required for dependency vulnerability scanning."
    Write-Host "Install: cargo install cargo-audit"
    exit 1
}

Write-Host "=== Dependency Audit ==="
cargo audit
if ($LASTEXITCODE -ne 0) {
    Write-Host ""
    Write-Host "WARNING: Vulnerable dependencies detected."
    Write-Host "Review the report above and update affected crates."
    exit $LASTEXITCODE
}

Write-Host "No known vulnerabilities found."
