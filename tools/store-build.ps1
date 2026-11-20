# Builds the Microsoft Store package (MSIX). The Store build has no updater: the Store delivers updates.
#
# For the Store, pass the three values from Partner Center > Product identity:
#   tools/store-build.ps1 -Name 12345Publisher.BrightPause -Publisher "CN=..." -PublisherDisplayName "..."
# Without them it builds an unsigned local test package, under a test identity and the dev app
# identifier, that installs with: Add-AppxPackage -Path <msix> -AllowUnsigned
param(
  [string]$Name = 'BrightPause.Test',
  [string]$Publisher = 'CN=BrightPause Test, OID.2.25.311729368913984317654407730594956997722=1',
  [string]$PublisherDisplayName = 'BrightPause Test',
  [string]$TauriConfig = '',
  [string]$Out = 'C:\Temp\brightpause-store'
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
$test = $Name -eq 'BrightPause.Test'
if ($test -and -not $TauriConfig) { throw 'A test package needs -TauriConfig with a separate identifier, so it never shares data with an installed BrightPause.' }

# MSIX versions are four numbers, the first not 0 and the last left at 0 for the Store: 1.2.3 becomes 1.2.3.0.
$v = (Get-Content "$root\src-tauri\tauri.conf.json" -Raw | ConvertFrom-Json).version -split '\.' | ForEach-Object { [int]$_ }
if ($v[0] -lt 1) { throw "The Store needs version 1.0.0 or later" }
$version = "$($v[0]).$($v[1]).$($v[2]).0"

$sdk = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin\*\x64\makeappx.exe' | Sort-Object FullName | Select-Object -Last 1
if (-not $sdk) { throw 'makeappx.exe not found: install the Windows 10/11 SDK.' }

Push-Location $root
try {
  $buildArgs = @('tauri', 'build', '--no-bundle', '--features', 'store')
  if ($TauriConfig) { $buildArgs += @('--config', $TauriConfig) }
  & npx @buildArgs
  if ($LASTEXITCODE) { throw "build failed ($LASTEXITCODE)" }
} finally { Pop-Location }

$layout = Join-Path $Out 'layout'
if (Test-Path $layout) { Remove-Item $layout -Recurse -Force }
New-Item -ItemType Directory -Force "$layout\assets" | Out-Null
Copy-Item "$root\src-tauri\target\release\brightpause.exe" $layout
$icons = "$root\src-tauri\icons"
foreach ($f in 'Square44x44Logo.png', 'Square150x150Logo.png', 'Square310x310Logo.png', 'StoreLogo.png') { Copy-Item "$icons\$f" "$layout\assets\$f" }
# Unplated 44 px icon for the taskbar and Start list, without the tile backplate.
Copy-Item "$icons\Square44x44Logo.png" "$layout\assets\Square44x44Logo.targetsize-44_altform-unplated.png"
Add-Type -AssemblyName System.Drawing
$src = [System.Drawing.Image]::FromFile("$icons\source.png")
$wide = New-Object System.Drawing.Bitmap 310, 150
$g = [System.Drawing.Graphics]::FromImage($wide); $g.InterpolationMode = 'HighQualityBicubic'
$g.DrawImage($src, 95, 15, 120, 120); $g.Dispose(); $src.Dispose()
$wide.Save("$layout\assets\Wide310x150Logo.png"); $wide.Dispose()

$display = if ($test) { 'BrightPause Test' } else { 'BrightPause' }
$manifest = (Get-Content "$PSScriptRoot\msix\AppxManifest.xml" -Raw).
  Replace('{NAME}', $Name).Replace('{PUBLISHER}', [Security.SecurityElement]::Escape($Publisher)).
  Replace('{PUBLISHER_DISPLAY_NAME}', [Security.SecurityElement]::Escape($PublisherDisplayName)).
  Replace('{DISPLAY_NAME}', $display).Replace('{VERSION}', $version)
[IO.File]::WriteAllText("$layout\AppxManifest.xml", $manifest, (New-Object Text.UTF8Encoding $false))

$msix = Join-Path $Out "BrightPause_$($version)_x64.msix"
& $sdk.FullName pack /d $layout /p $msix /o | Select-Object -Last 2
if ($LASTEXITCODE) { throw "makeappx failed ($LASTEXITCODE)" }
"$msix  SHA256 $((Get-FileHash $msix).Hash)"
