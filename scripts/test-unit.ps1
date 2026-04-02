param(
    [string]$Filter = ""
)

$ErrorActionPreference = "Stop"

$testArgs = @("test", "--lib")
if ($Filter) {
    $testArgs += "--", $Filter
}

Write-Host "=== Running Unit Tests ==="
& cargo @testArgs
if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: Unit tests failed."
    exit $LASTEXITCODE
}

Write-Host ""
Write-Host "All unit tests passed."
