# Pelna przebudowa: clean -> prep:brand -> nowy certyfikat podpisu -> MSI podpisany.
$ErrorActionPreference = 'Stop'
$proj = Split-Path $PSScriptRoot -Parent
Set-Location $proj

& (Join-Path $PSScriptRoot 'clean-build.ps1') -All
npm run prep:brand
& (Join-Path $PSScriptRoot 'New-CodeSigningCertificate.ps1') -Force
$env:CSC_LINK = (Resolve-Path (Join-Path $proj 'build\codesign.pfx')).Path
$env:CSC_KEY_PASSWORD = (Get-Content -LiteralPath (Join-Path $proj 'build\codesign-password.txt') -Raw).Trim()
npm run make
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host 'OK: rebuild-all -> out\createcrafts-installer\createcrafts-installer.msi'
