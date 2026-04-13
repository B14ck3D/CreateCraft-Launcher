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
