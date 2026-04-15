$nsisDir = Join-Path $PSScriptRoot "..\src-tauri\target\release\bundle\nsis"
$msiDir  = Join-Path $PSScriptRoot "..\src-tauri\target\release\bundle\msi"

# Try NSIS first, then MSI as fallback
$bundleDir = if (Test-Path $nsisDir) { $nsisDir } else { $msiDir }
$ext       = if (Test-Path $nsisDir) { "*.exe" } else { "*.msi" }
$outExt    = if (Test-Path $nsisDir) { ".exe" } else { ".msi" }
$target    = Join-Path $bundleDir "CreateCrafts-installer$outExt"

$found = Get-ChildItem -Path $bundleDir -Filter $ext -ErrorAction SilentlyContinue |
         Where-Object { $_.Name -ne "CreateCrafts-installer$outExt" } |
         Select-Object -First 1

if (-not $found) {
    if (Test-Path $target) {
        Write-Host "OK: $target (already named correctly)"
    } else {
        Write-Warning "Brak pliku instalatora w $bundleDir"
    }
    exit 0
}

if (Test-Path $target) { Remove-Item $target -Force }
Rename-Item -Path $found.FullName -NewName "CreateCrafts-installer$outExt" -Force
Write-Host "OK: $(Join-Path $bundleDir "CreateCrafts-installer$outExt")"
