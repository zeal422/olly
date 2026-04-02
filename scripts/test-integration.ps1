param(
    [string]$Filter = ""
)

$ErrorActionPreference = "Stop"

$testArgs = @("test", "--test", "*")
if ($Filter) {
    $testArgs += "--", $Filter
}

Write-Host "=== Running Integration Tests ==="
& cargo @testArgs
if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: Integration tests failed."
    exit $LASTEXITCODE
}

Write-Host ""
Write-Host "All integration tests passed."
