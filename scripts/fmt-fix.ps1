param(
    [switch]$AllowDirty,
    [switch]$NoClippy
)

$ErrorActionPreference = "Stop"

Write-Host "=== Formatting Fix ==="
Write-Host "Applying cargo fmt..."
cargo fmt --all
if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: cargo fmt failed."
    exit $LASTEXITCODE
}
Write-Host "Formatting applied."

if (-not $NoClippy) {
    Write-Host ""
    Write-Host "=== Clippy Auto-Fix ==="
    $fixArgs = @("clippy", "--all-targets", "--all-features", "--fix")
    if ($AllowDirty) {
        $fixArgs += "--allow-dirty", "--allow-staged"
    }
    $fixArgs += "--", "-D", "warnings"

    & cargo @fixArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Host ""
        Write-Host "WARNING: clippy --fix could not resolve all issues."
        Write-Host "Run './scripts/check.ps1' to see remaining problems."
        exit $LASTEXITCODE
    }
    Write-Host "Clippy fixes applied."
}

Write-Host ""
Write-Host "Done. Run './scripts/check.ps1' to verify."
