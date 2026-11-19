# Microsoft Store screenshots (1920x1080), per language, rendered from the UI with
# tools/mock-tauri.js in headless Edge. Nothing is captured from the screen.
param([string]$Out = 'C:\Temp\brightpause-store\screenshots', [string[]]$Languages = @('en', 'fr', 'es', 'de'))
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$site = Join-Path $env:TEMP 'brightpause-store-preview'
Remove-Item $site -Recurse -Force -ErrorAction SilentlyContinue
Copy-Item "$root\ui" $site -Recurse
Copy-Item "$PSScriptRoot\mock-tauri.js" $site
foreach ($l in $Languages) { Copy-Item "$root\src-tauri\i18n\$l.json" "$site\i18n-$l.json" }
foreach ($f in Get-ChildItem $site -Filter *.html) {
  (Get-Content $f.FullName -Raw).Replace('<script type="module"', '<script src="mock-tauri.js"></script><script type="module"') | Set-Content $f.FullName -NoNewline
}
New-Item -ItemType Directory -Force $Out | Out-Null
$raw = Join-Path $env:TEMP 'brightpause-store-raw'; New-Item -ItemType Directory -Force $raw | Out-Null
$server = Start-Process python -ArgumentList '-m', 'http.server', '8791', '--bind', '127.0.0.1', '--directory', $site -PassThru -WindowStyle Hidden
Start-Sleep 2
$edge = 'C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe'
function Render($file, $url, $size) {
  $profile = Join-Path $env:TEMP "brightpause-edge-$([guid]::NewGuid())"
  & $edge --headless=new --disable-gpu --hide-scrollbars --user-data-dir=$profile --virtual-time-budget=3000 "--window-size=$size" "--screenshot=$file" "http://127.0.0.1:8791/$url" 2>$null | Out-Null
  Remove-Item $profile -Recurse -Force -ErrorAction SilentlyContinue
}
Add-Type -AssemblyName System.Drawing
try {
  foreach ($l in $Languages) {
    Render "$Out\$l-1-break-eye.png" "break.html?lang=$l&kind=mini&theme=dark" '1920,1080'
    Render "$Out\$l-2-break-move.png" "break.html?lang=$l&kind=long&theme=dark" '1920,1080'
    # The settings window, top part, set on a calm backdrop at Store size.
    Render "$raw\$l-settings.png" "settings.html?lang=$l&theme=light&update=store" '720,960'
    $canvas = New-Object System.Drawing.Bitmap 1920, 1080
    $g = [System.Drawing.Graphics]::FromImage($canvas)
    $g.Clear([System.Drawing.Color]::FromArgb(0x25, 0x28, 0x4a))
    $win = [System.Drawing.Image]::FromFile("$raw\$l-settings.png")
    $g.DrawImage($win, 600, 60, 720, 960); $win.Dispose(); $g.Dispose()
    $canvas.Save("$Out\$l-3-settings.png"); $canvas.Dispose()
    "$l done"
  }
} finally {
  Stop-Process $server.Id -Force
  Remove-Item $raw -Recurse -Force -ErrorAction SilentlyContinue
}
