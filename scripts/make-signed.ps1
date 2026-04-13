# Ustawia CSC_LINK / CSC_KEY_PASSWORD i uruchamia pelny build MSI z podpisem.
$ErrorActionPreference = 'Stop'
$proj = Split-Path $PSScriptRoot -Parent
$pfxPath = Join-Path $proj 'build\codesign.pfx'
$pwPath = Join-Path $proj 'build\codesign-password.txt'

if (-not (Test-Path $pfxPath)) {
  & (Join-Path $PSScriptRoot 'New-CodeSigningCertificate.ps1')
}
if (-not (Test-Path $pwPath)) {
  throw "Brak hasla: $pwPath — uruchom scripts\New-CodeSigningCertificate.ps1"
}

$env:CSC_LINK = (Resolve-Path $pfxPath).Path
$env:CSC_KEY_PASSWORD = (Get-Content -LiteralPath $pwPath -Raw).Trim()

Set-Location $proj
npm run make
