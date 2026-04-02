$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$srcDir = Join-Path $repoRoot "src"

# Collect all .rs files and their line counts
$files = Get-ChildItem -Path $srcDir -Recurse -Filter "*.rs" | Sort-Object FullName
$total = 0
$groups = @{}

Write-Host "=== Lines of Code by Module ==="
Write-Host ""
Write-Host ("{0,-50} {1,8}" -f "File", "Lines")
Write-Host ("{0,-50} {1,8}" -f ("-" * 50), ("-" * 8))

foreach ($file in $files) {
    $relative = $file.FullName.Substring($repoRoot.Path.Length + 1).Replace("\", "/")
    $lines = (Get-Content $file.FullName).Count
    $total += $lines

    # Group by top-level module
    $parts = $relative.Split("/")
    if ($parts.Length -ge 2) {
        $group = $parts[0..1] -join "/"
    } else {
        $group = $parts[0]
    }
    if (-not $groups.ContainsKey($group)) {
        $groups[$group] = 0
    }
    $groups[$group] += $lines

    Write-Host ("{0,-50} {1,8}" -f $relative, $lines)
}

Write-Host ("{0,-50} {1,8}" -f ("-" * 50), ("-" * 8))
Write-Host ("{0,-50} {1,8}" -f "TOTAL", $total)
Write-Host ""
Write-Host "=== By Module Group ==="
Write-Host ""
Write-Host ("{0,-30} {1,8}" -f "Module", "Lines")
Write-Host ("{0,-30} {1,8}" -f ("-" * 30), ("-" * 8))

$groups.GetEnumerator() | Sort-Object Value -Descending | ForEach-Object {
    Write-Host ("{0,-30} {1,8}" -f $_.Key, $_.Value)
}
