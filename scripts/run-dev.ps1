param(
    [string]$Model = "",
    [string]$Url = "",
    [switch]$Release
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")

$buildArgs = @("run")
if ($Release) {
    $buildArgs += "--release"
}
$buildArgs += "--"

$defaultModel = "llama3.2:latest"
$defaultUrl = "http://localhost:11434"

$model = if ($Model) { $Model } else { $defaultModel }
$url = if ($Url) { $Url } else { $defaultUrl }

$buildArgs += "--url", $url, "--model", $model

Write-Host "=== Running Olly (dev) ==="
Write-Host "Model: $model"
Write-Host "URL: $url"
Write-Host ""

Set-Location $repoRoot
& cargo @buildArgs
