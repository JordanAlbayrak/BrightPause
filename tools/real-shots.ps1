# Screenshots of the real app: break backgrounds (main and one side screen) and the settings
# window's background picker and update card. Run it on a build with its own identifier, so
# its settings and data stay apart from an installed copy.
param(
  [Parameter(Mandatory)][string]$Exe,
  [Parameter(Mandatory)][string]$DataDir,
  [string]$Out = "$PSScriptRoot\..\docs\shots",
  [string]$UpdateFeed
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms, System.Drawing, UIAutomationClient, UIAutomationTypes
Add-Type 'using System;using System.Runtime.InteropServices;public static class Dpi{[DllImport("user32.dll")]public static extern bool SetProcessDPIAware();[DllImport("dwmapi.dll")]public static extern int DwmGetWindowAttribute(IntPtr h,int a,out RECT r,int s);public struct RECT{public int L,T,R,B;}}'
[Dpi]::SetProcessDPIAware() | Out-Null
$UIA = [System.Windows.Automation.AutomationElement]
New-Item -ItemType Directory -Force $Out, $DataDir | Out-Null

# Windows 11 rounds window corners; paint over what shows through them.
function Clear-Corners([System.Drawing.Bitmap]$b, [int]$r = 9) {
  $c = $b.GetPixel($r + 2, 2)
  for ($y = 0; $y -lt $r; $y++) { for ($x = 0; $x -lt $r; $x++) {
    if ([math]::Sqrt([math]::Pow($r - $x, 2) + [math]::Pow($r - $y, 2)) -gt $r - 1) {
      foreach ($p in @(@($x, $y), @(($b.Width - 1 - $x), $y), @($x, ($b.Height - 1 - $y)), @(($b.Width - 1 - $x), ($b.Height - 1 - $y)))) { $b.SetPixel($p[0], $p[1], $c) }
    }
  } }
}

function Shot([string]$Name, [System.Drawing.Rectangle]$R, [int]$MaxWidth = 1600, [switch]$Window) {
  $bmp = New-Object System.Drawing.Bitmap $R.Width, $R.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($R.Left, $R.Top, 0, 0, $bmp.Size)
  $g.Dispose()
  if ($Window) { Clear-Corners $bmp }
  if ($R.Width -gt $MaxWidth) {
    $h = [int]($R.Height * $MaxWidth / $R.Width)
    $small = New-Object System.Drawing.Bitmap $MaxWidth, $h
    $g = [System.Drawing.Graphics]::FromImage($small)
    $g.InterpolationMode = 'HighQualityBicubic'
    $g.DrawImage($bmp, 0, 0, $MaxWidth, $h)
    $g.Dispose(); $bmp.Dispose(); $bmp = $small
  }
  $bmp.Save((Join-Path $Out $Name), [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
}

# Full-screen shots drop a thin edge, so nothing next to the break window shows.
function Inset([System.Drawing.Rectangle]$R) { New-Object System.Drawing.Rectangle ($R.X + 8), ($R.Y + 8), ($R.Width - 16), ($R.Height - 16) }

function Set-Background([string]$Value) {
  $path = Join-Path $DataDir 'settings.json'
  $s = if (Test-Path $path) { Get-Content $path -Raw | ConvertFrom-Json -AsHashtable } else { @{} }
  $s.background = $Value
  $s | ConvertTo-Json | Set-Content $path
}

function Start-App {
  $p = Start-Process $Exe -PassThru
  Start-Sleep 4
  $p
}

function Stop-App($p) {
  & $Exe --quit
  Start-Sleep 2
  if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force }
}

function App-Window([int]$ProcessId) {
  $cond = New-Object System.Windows.Automation.PropertyCondition($UIA::ProcessIdProperty, $ProcessId)
  $UIA::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
}

# The visible frame only: BoundingRectangle includes the invisible resize border, which shows what is behind.
function Rect($e) {
  $r = New-Object Dpi+RECT
  [Dpi]::DwmGetWindowAttribute([IntPtr]$e.Current.NativeWindowHandle, 9, [ref]$r, 16) | Out-Null
  New-Object System.Drawing.Rectangle ($r.L + 1), ($r.T + 1), ($r.R - $r.L - 2), ($r.B - $r.T - 2)
}

function Find-Text($root, [string]$Name) {
  $cond = New-Object System.Windows.Automation.PropertyCondition($UIA::NameProperty, $Name)
  $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
}

function Scroll-To($root, [string]$Name) {
  for ($i = 0; $i -lt 10 -and -not ($e = Find-Text $root $Name); $i++) { Start-Sleep -Milliseconds 500 }
  if (-not $e) { Write-Warning "not found: $Name"; return }
  $pattern = $null
  if ($e.TryGetCurrentPattern([System.Windows.Automation.ScrollItemPattern]::Pattern, [ref]$pattern)) { $pattern.ScrollIntoView() }
  Start-Sleep 1
}

function Press($root, [string]$Name) {
  $e = Find-Text $root $Name
  if (-not $e) { Write-Warning "button not found: $Name"; return }
  $e.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
}

$screens = [System.Windows.Forms.Screen]::AllScreens
$primary = [System.Windows.Forms.Screen]::PrimaryScreen
$side = $screens | Where-Object { -not $_.Primary } | Select-Object -First 1

foreach ($bg in 'color:#173a3d', 'color:#33243f', 'image', 'theme') {
  $name = ($bg -replace '^color:', '' -replace '#', '')
  Set-Background $bg
  $p = Start-App
  & $Exe --break-now; Start-Sleep 4
  Shot "real-bg-$name.png" (Inset $primary.Bounds)
  if ($side) { Shot "real-bg-$name-side.png" (Inset $side.Bounds) 800 }
  & $Exe --skip; Start-Sleep 1
  Stop-App $p
}

Set-Background 'color:#25284a'
$p = Start-App
& $Exe --settings; Start-Sleep 4
$w = App-Window $p.Id
Shot 'real-settings.png' (Rect $w) -Window
Scroll-To $w 'Break screen'
Shot 'real-settings-backgrounds.png' (Rect $w) -Window
Scroll-To $w 'Updates'
Shot 'real-update-idle.png' (Rect $w) -Window
if ($UpdateFeed) {
  Press $w 'Check for updates'; Start-Sleep 3
  Scroll-To $w 'Updates'
  Shot 'real-update-result.png' (Rect $w) -Window
}
Stop-App $p
