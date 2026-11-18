# Renders the UI with tools/mock-tauri.js in headless Edge and saves screenshots.
param([string]$Out = "$PSScriptRoot\..\docs\shots")
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$site = Join-Path $env:TEMP 'brightpause-preview'
Remove-Item $site -Recurse -Force -ErrorAction SilentlyContinue
Copy-Item "$root\ui" $site -Recurse
Copy-Item "$PSScriptRoot\mock-tauri.js" $site
foreach ($f in Get-ChildItem $site -Filter *.html) {
  (Get-Content $f.FullName -Raw).Replace('<script type="module"', '<script src="mock-tauri.js"></script><script type="module"') | Set-Content $f.FullName -NoNewline
}
New-Item -ItemType Directory -Force $Out | Out-Null
$server = Start-Process python -ArgumentList '-m', 'http.server', '8790', '--bind', '127.0.0.1', '--directory', $site -PassThru -WindowStyle Hidden
Start-Sleep 2
$edge = 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'
$shots = @(
  @('break-eye-light.png', 'break.html?kind=mini&theme=light#quiet', '1600,900'),
  @('break-eye-dark.png', 'break.html?kind=mini&theme=dark#quiet', '1600,900'),
  @('break-move-light.png', 'break.html?kind=long&theme=light#quiet', '1600,900'),
  @('break-strict-dark.png', 'break.html?kind=long&theme=dark&strict=1#quiet', '1600,900'),
  @('settings-light.png', 'settings.html?theme=light', '720,1700'),
  @('settings-dark.png', 'settings.html?theme=dark', '720,1700')
)
try {
  foreach ($s in $shots) {
    $profile = Join-Path $env:TEMP "brightpause-edge-$([guid]::NewGuid())"
    & $edge --headless=new --disable-gpu --hide-scrollbars --user-data-dir=$profile --virtual-time-budget=3000 "--window-size=$($s[2])" "--screenshot=$Out\$($s[0])" "http://127.0.0.1:8790/$($s[1])" 2>$null | Out-Null
    Remove-Item $profile -Recurse -Force -ErrorAction SilentlyContinue
    "$($s[0])"
  }
} finally {
  Stop-Process $server.Id -Force
}
