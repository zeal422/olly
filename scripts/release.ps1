$ErrorActionPreference = "Stop"

./scripts/check.ps1

if (-not (Get-Command git-cliff -ErrorAction SilentlyContinue)) {
    Write-Host "git-cliff is required to generate the changelog."
    Write-Host "Install: cargo install git-cliff"
    exit 1
}

git-cliff --config .git-cliff.toml --output CHANGELOG.md
cargo package

Write-Host "Release preparation complete."
Write-Host "Next: tag with vX.Y.Z and push to trigger .github/workflows/release.yml."
