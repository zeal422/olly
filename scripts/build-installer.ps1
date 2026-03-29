param(
    [string]$Version = "",
    [string]$Target = "x86_64-pc-windows-msvc",
    [string]$OutputDir = "dist/windows-installer",
    [switch]$SkipBuild,
    [switch]$KeepStaging
)

$ErrorActionPreference = "Stop"

function Get-ProjectVersion {
    param([string]$CargoTomlPath)
    $line = Select-String -Path $CargoTomlPath -Pattern '^\s*version\s*=\s*"([^"]+)"' | Select-Object -First 1
    if (-not $line) {
        throw "Could not read project version from $CargoTomlPath."
    }
    return $line.Matches[0].Groups[1].Value
}

function Get-IsccPath {
    $cmd = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($cmd) {
        return $cmd.Source
    }

    $programFilesX86 = [Environment]::GetFolderPath("ProgramFilesX86")
    $programFiles = [Environment]::GetFolderPath("ProgramFiles")
    $localPrograms = Join-Path $env:LOCALAPPDATA "Programs"
    $candidates = @(
        (Join-Path $programFilesX86 "Inno Setup 6\ISCC.exe"),
        (Join-Path $programFiles "Inno Setup 6\ISCC.exe"),
        (Join-Path $localPrograms "Inno Setup 6\ISCC.exe")
    )

    foreach ($candidate in $candidates) {
        if (Test-Path $candidate) {
            return $candidate
        }
    }

    throw "Inno Setup compiler (ISCC.exe) not found. Install it with: winget install --id JRSoftware.InnoSetup -e --silent"
}

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$cargoToml = Join-Path $repoRoot "Cargo.toml"
$installerScript = Join-Path $repoRoot "installer\olly.iss"
$iconPath = Join-Path $repoRoot "assets\icon\olly.ico"

if (-not (Test-Path $installerScript)) {
    throw "Installer script not found at $installerScript"
}

if (-not (Test-Path $iconPath)) {
    throw "Icon file not found at $iconPath"
}

if ([string]::IsNullOrWhiteSpace($Version)) {
    $Version = Get-ProjectVersion -CargoTomlPath $cargoToml
}

if (-not $SkipBuild) {
    cargo build --release --target $Target
    if ($LASTEXITCODE -ne 0) {
        throw "Cargo build failed."
    }
}

$targetBinary = Join-Path $repoRoot "target\$Target\release\olly.exe"
if (-not (Test-Path $targetBinary)) {
    $fallbackBinary = Join-Path $repoRoot "target\release\olly.exe"
    if (Test-Path $fallbackBinary) {
        $targetBinary = $fallbackBinary
    } else {
        throw "Could not find release binary at $targetBinary or $fallbackBinary"
    }
}

$outputRoot = Join-Path $repoRoot $OutputDir
$stageDir = Join-Path ([System.IO.Path]::GetTempPath()) ("olly-installer-staging-" + [Guid]::NewGuid().ToString("N"))

try {
    New-Item -ItemType Directory -Path $stageDir -Force | Out-Null

    Copy-Item -LiteralPath $targetBinary -Destination (Join-Path $stageDir "olly.exe") -Force
    Copy-Item -LiteralPath (Join-Path $repoRoot "README.md") -Destination (Join-Path $stageDir "README.md") -Force
    Copy-Item -LiteralPath (Join-Path $repoRoot "LICENSE") -Destination (Join-Path $stageDir "LICENSE") -Force

    $isccPath = Get-IsccPath

    $env:OLLY_APP_VERSION = $Version
    $env:OLLY_SOURCE_DIR = (Resolve-Path $stageDir).Path
    $env:OLLY_OUTPUT_DIR = (Resolve-Path $outputRoot).Path
    $env:OLLY_ICON_PATH = (Resolve-Path $iconPath).Path

    & $isccPath $installerScript
    if ($LASTEXITCODE -ne 0) {
        throw "Installer build failed."
    }

    $installerPath = Join-Path $outputRoot "Olly-Setup-$Version.exe"
    if (-not (Test-Path $installerPath)) {
        throw "Expected installer output not found: $installerPath"
    }

    Write-Host "Installer created: $installerPath"
}
finally {
    if (-not $KeepStaging -and (Test-Path $stageDir)) {
        Remove-Item -LiteralPath $stageDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}
