$ErrorActionPreference = "Stop"

$hookPath = ".git/hooks/pre-commit"
$content = @'
#!/usr/bin/env bash
set -euo pipefail
./scripts/check.sh
'@

Set-Content -Path $hookPath -Value $content -NoNewline
Write-Host "Installed pre-commit hook at $hookPath"
