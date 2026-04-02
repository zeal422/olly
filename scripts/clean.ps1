param(
    [switch]$Yes
)

$ErrorActionPreference = "Stop"

if (-not $Yes) {
    Write-Host "This will remove all build artifacts from target/."
    Write-Host "Proceed? [y/N] "
    $response = Read-Host
    if ($response -ne "y" -and $response -ne "Y") {
        Write-Host "Cancelled."
        exit 0
    }
}

Write-Host "=== Cleaning Build Artifacts ==="
cargo clean
if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: cargo clean failed."
    exit $LASTEXITCODE
}

Write-Host "Done. All build artifacts removed."
