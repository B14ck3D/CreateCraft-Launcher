# Generuje build/icon.png (256x256) z public/logomain.png — ikona aplikacji i electron-builder (Windows).
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$proj = Split-Path $PSScriptRoot -Parent
$src = Join-Path $proj 'public\logomain.png'
$outDir = Join-Path $proj 'build'
$outPng = Join-Path $outDir 'icon.png'

if (-not (Test-Path $src)) { throw "Brak pliku: $src" }
New-Item -ItemType Directory -Path $outDir -Force | Out-Null

$img = [System.Drawing.Image]::FromFile($src)
$size = 256
$bmp = New-Object System.Drawing.Bitmap($size, $size)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
$m = [Math]::Min($img.Width, $img.Height)
$sx = [int](($img.Width - $m) / 2)
$sy = [int](($img.Height - $m) / 2)
$g.DrawImage(
  $img,
  [System.Drawing.Rectangle]::new(0, 0, $size, $size),
  $sx,
  $sy,
  $m,
  $m,
  [System.Drawing.GraphicsUnit]::Pixel
)
$g.Dispose()
$img.Dispose()
$bmp.Save($outPng, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
$pub = Join-Path $proj 'public\icon.png'
Copy-Item -LiteralPath $outPng -Destination $pub -Force
Write-Host "OK: $outPng + $pub"

$brandSrv = Join-Path $proj 'branding\servers.dat'
$outSrv = Join-Path $outDir 'createcrafts-servers-default.dat'
if (-not (Test-Path $brandSrv)) { throw "Brak szablonu listy serwerow (NBT): $brandSrv" }
Copy-Item -LiteralPath $brandSrv -Destination $outSrv -Force
Write-Host "OK: $outSrv (servers.dat 1:1 z branding)"

$brandPlain = Join-Path $proj 'branding\launcher-mods-key'
$outPlain = Join-Path $outDir 'launcher-mods-key'
if (Test-Path $brandPlain) {
  Copy-Item -LiteralPath $brandPlain -Destination $outPlain -Force
  Write-Host "OK: $outPlain (mods API key plain text from branding -> do not commit this file)"
}

$brandModsKey = Join-Path $proj 'branding\launcher-mods-key.enc'
$outModsKey = Join-Path $outDir 'launcher-mods-key.enc'
if (Test-Path $brandModsKey) {
  Copy-Item -LiteralPath $brandModsKey -Destination $outModsKey -Force
  Write-Host "OK: $outModsKey (mods API key .enc from branding -> MSI for all downloaders)"
} elseif (-not (Test-Path $brandPlain)) {
  Write-Host "INFO: Missing $brandModsKey and $brandPlain - set LAUNCHER_MODS_API_KEY for make, or: echo KEY | node scripts/embed-mods-api-key.cjs --stdin --branding"
}
