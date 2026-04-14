
$ErrorActionPreference = 'Stop'
$proj = Split-Path $PSScriptRoot -Parent
Set-Location $proj

& (Join-Path $PSScriptRoot 'clean-build.ps1') -All
npm run prep:brand
npm run make
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host 'OK: rebuild-all -> out\createcrafts-installer\createcrafts-installer.msi'
