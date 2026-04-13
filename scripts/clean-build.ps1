# Usuwa artefakty buildu (out, .vite, dist). Z -All usuwa tez lokalny certyfikat podpisu w build/.
param([switch]$All)

$ErrorActionPreference = 'Stop'
$proj = Split-Path $PSScriptRoot -Parent

foreach ($name in @('out', '.vite', 'dist')) {
  $p = Join-Path $proj $name
  if (Test-Path $p) {
    Remove-Item -LiteralPath $p -Recurse -Force
    Write-Host "Removed: $p"
  }
}

if ($All) {
  foreach ($rel in @('build\codesign.pfx', 'build\codesign-password.txt')) {
    $f = Join-Path $proj $rel
    if (Test-Path $f) {
      Remove-Item -LiteralPath $f -Force
      Write-Host "Removed: $f"
    }
  }
}

Write-Host 'OK: clean-build'
