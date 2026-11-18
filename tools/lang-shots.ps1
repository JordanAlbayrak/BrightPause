# Screenshots of the settings window (top to bottom) and a break, once per language, to check
# that translated text fits. Run it on a build with its own identifier (see real-shots.ps1).
param(
  [Parameter(Mandatory)][string]$Exe,
  [Parameter(Mandatory)][string]$DataDir,
  [string[]]$Languages = @('en', 'fr', 'es', 'de'),
  [string]$Out = "$PSScriptRoot\..\docs\local-shots"
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms, System.Drawing, UIAutomationClient, UIAutomationTypes
Add-Type 'using System;using System.Runtime.InteropServices;public static class Win{[DllImport("user32.dll")]public static extern bool SetProcessDPIAware();[DllImport("dwmapi.dll")]public static extern int DwmGetWindowAttribute(IntPtr h,int a,out RECT r,int s);public struct RECT{public int L,T,R,B;}public struct POINT{public int X,Y;}[DllImport("user32.dll")]public static extern IntPtr WindowFromPoint(POINT p);[DllImport("user32.dll")]public static extern uint GetWindowThreadProcessId(IntPtr h,out uint pid);[DllImport("user32.dll")]public static extern bool SetForegroundWindow(IntPtr h);[DllImport("user32.dll")]public static extern IntPtr GetAncestor(IntPtr h,uint f);}'
[Win]::SetProcessDPIAware() | Out-Null
$UIA = [System.Windows.Automation.AutomationElement]
New-Item -ItemType Directory -Force $Out, $DataDir | Out-Null

# True when every sampled point of the area shows a window of process $Owner, so a capture
# cannot include another app's window.
function Test-Owned([System.Drawing.Rectangle]$R, [int]$Owner) {
  foreach ($fx in 0.1, 0.5, 0.9) { foreach ($fy in 0.1, 0.5, 0.9) {
    $pt = New-Object Win+POINT
    $pt.X = [int]($R.X + $R.Width * $fx); $pt.Y = [int]($R.Y + $R.Height * $fy)
    $owner = 0
    [Win]::GetWindowThreadProcessId([Win]::GetAncestor([Win]::WindowFromPoint($pt), 2), [ref]$owner) | Out-Null
    if ($owner -ne $Owner) { return $false }
  } }
  $true
}

function Save-Shot([string]$Name, [System.Drawing.Rectangle]$R, [switch]$Window) {
  if (-not (Test-Owned $R $script:appPid)) { Write-Warning "$Name skipped: another window covers the app"; return }
  $bmp = New-Object System.Drawing.Bitmap $R.Width, $R.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($R.Left, $R.Top, 0, 0, $bmp.Size)
  $g.Dispose()
  if ($Window) {
    # Windows 11 rounds window corners; paint over what shows through them.
    $c = $bmp.GetPixel(11, 2)
    for ($y = 0; $y -lt 9; $y++) { for ($x = 0; $x -lt 9; $x++) {
      if ([math]::Sqrt([math]::Pow(9 - $x, 2) + [math]::Pow(9 - $y, 2)) -gt 8) {
        foreach ($p in @(@($x, $y), @(($bmp.Width - 1 - $x), $y), @($x, ($bmp.Height - 1 - $y)), @(($bmp.Width - 1 - $x), ($bmp.Height - 1 - $y)))) { $bmp.SetPixel($p[0], $p[1], $c) }
      }
    } }
  }
  if ($R.Width -gt 1600) {
    $h = [int]($R.Height * 1600 / $R.Width)
    $small = New-Object System.Drawing.Bitmap 1600, $h
    $g = [System.Drawing.Graphics]::FromImage($small)
    $g.InterpolationMode = 'HighQualityBicubic'
    $g.DrawImage($bmp, 0, 0, 1600, $h)
    $g.Dispose(); $bmp.Dispose(); $bmp = $small
  }
  $bmp.Save((Join-Path $Out $Name), [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
}

# The visible frame minus its 1-pixel edge: the plain window rectangle includes the invisible
# resize border, which shows what is behind the window.
function Frame($e) {
  $r = New-Object Win+RECT
  [Win]::DwmGetWindowAttribute([IntPtr]$e.Current.NativeWindowHandle, 9, [ref]$r, 16) | Out-Null
  New-Object System.Drawing.Rectangle ($r.L + 1), ($r.T + 1), ($r.R - $r.L - 2), ($r.B - $r.T - 2)
}

function Set-Language([string]$Lang) {
  $path = Join-Path $DataDir 'settings.json'
  $s = if (Test-Path $path) { Get-Content $path -Raw | ConvertFrom-Json -AsHashtable } else { @{} }
  $s.language = $Lang
  $s.start_at_login = $false
  $s | ConvertTo-Json | Set-Content $path
}

$primary = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$inset = New-Object System.Drawing.Rectangle ($primary.X + 8), ($primary.Y + 8), ($primary.Width - 16), ($primary.Height - 16)
foreach ($lang in $Languages) {
  Set-Language $lang
  $p = Start-Process $Exe -PassThru
  $script:appPid = $p.Id
  Start-Sleep 3
  & $Exe --settings
  Start-Sleep 4
  $cond = New-Object System.Windows.Automation.PropertyCondition($UIA::ProcessIdProperty, $p.Id)
  $w = $UIA::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
  [Win]::SetForegroundWindow([IntPtr]$w.Current.NativeWindowHandle) | Out-Null
  Start-Sleep 1
  # Page through the settings by scrolling known elements into view (their HTML ids).
  foreach ($id in 'taken', 'cadence', 'tiles', 'language', 'check-updates') {
    $e = $w.FindFirst([System.Windows.Automation.TreeScope]::Descendants, (New-Object System.Windows.Automation.PropertyCondition($UIA::AutomationIdProperty, $id)))
    $sp = $null
    if ($e -and $e.TryGetCurrentPattern([System.Windows.Automation.ScrollItemPattern]::Pattern, [ref]$sp)) { $sp.ScrollIntoView(); Start-Sleep 1 }
    Save-Shot "lang-$lang-settings-$id.png" (Frame $w) -Window
  }
  & $Exe --break-now
  Start-Sleep 4
  Save-Shot "lang-$lang-break.png" $inset
  & $Exe --skip
  Start-Sleep 1
  & $Exe --quit
  $p.WaitForExit(10000) | Out-Null
}
Set-Language 'system'
