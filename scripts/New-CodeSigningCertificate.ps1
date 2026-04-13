# Self-signed Authenticode (test / wewnetrzny podpis). Dla publicznej dystrybucji uzyj certyfikatu z zaufanego CA.
# Zapis: build/codesign.pfx + build/codesign-password.txt (folder build/ jest w .gitignore).
param(
  [switch]$Force
)

$ErrorActionPreference = 'Stop'
$proj = Split-Path $PSScriptRoot -Parent
$buildDir = Join-Path $proj 'build'
$pfxPath = Join-Path $buildDir 'codesign.pfx'
$pwPath = Join-Path $buildDir 'codesign-password.txt'

New-Item -ItemType Directory -Path $buildDir -Force | Out-Null

if ((Test-Path $pfxPath) -and -not $Force) {
  Write-Host "Juz istnieje: $pfxPath (uruchom z -Force aby nadpisac)"
  exit 0
}

$chars = [char[]]([char]'A'..[char]'Z') + [char[]]([char]'a'..[char]'z') + [char[]]([char]'0'..[char]'9')
$plain = -join (1..48 | ForEach-Object { $chars | Get-Random })
$securePwd = ConvertTo-SecureString -String $plain -AsPlainText -Force

Write-Host 'Tworzenie certyfikatu Code Signing (self-signed)...'
$cert = New-SelfSignedCertificate `
  -Type CodeSigningCert `
  -Subject 'CN=CreateCrafts Launcher Code Signing' `
  -KeyAlgorithm RSA `
  -KeyLength 2048 `
  -HashAlgorithm SHA256 `
  -CertStoreLocation 'Cert:\CurrentUser\My' `
  -NotAfter (Get-Date).AddYears(5)

try {
  Export-PfxCertificate -Cert $cert -FilePath $pfxPath -Password $securePwd | Out-Null
}
finally {
  Remove-Item -Path "Cert:\CurrentUser\My\$($cert.Thumbprint)" -DeleteKey -Force -ErrorAction SilentlyContinue
}

$utf8 = New-Object System.Text.UTF8Encoding $false
[System.IO.File]::WriteAllText($pwPath, $plain, $utf8)

Write-Host "OK: $pfxPath"
Write-Host "Haslo zapisane: $pwPath"
Write-Host ''
Write-Host 'Self-signed: SmartScreen moze ostrzegac - do produkcji uzyj certyfikatu z zaufanego CA.'
Write-Host 'Build z podpisem: npm run make:signed'
