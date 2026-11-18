# Clicks through an NSIS installer with UI Automation (only inside the installer's own window),
# screenshots the finish page, unticks "Start with Windows" and "Run", and finishes.
# Use it on a build with its own identifier and product name, then uninstall it.
param([Parameter(Mandatory)][string]$Setup, [string]$Out = "$PSScriptRoot\..\docs\local-shots")
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing, UIAutomationClient, UIAutomationTypes
Add-Type 'using System;using System.Runtime.InteropServices;public static class Dpi{[DllImport("user32.dll")]public static extern bool SetProcessDPIAware();[DllImport("user32.dll")]public static extern IntPtr SendMessage(IntPtr h,uint m,IntPtr w,IntPtr l);[DllImport("user32.dll")]public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);[DllImport("dwmapi.dll")]public static extern int DwmGetWindowAttribute(IntPtr h,int a,out RECT r,int s);public struct RECT{public int L,T,R,B;}}'
[Dpi]::SetProcessDPIAware() | Out-Null
$UIA = [System.Windows.Automation.AutomationElement]
New-Item -ItemType Directory -Force $Out | Out-Null

function Window($p) {
  $cond = New-Object System.Windows.Automation.PropertyCondition($UIA::ProcessIdProperty, $p.Id)
  for ($i = 0; $i -lt 40; $i++) {
    $w = $UIA::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
    if ($w) { return $w }
    Start-Sleep -Milliseconds 250
  }
  throw 'installer window not found'
}

function Find($w, [string]$Pattern) {
  $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition) |
    Where-Object { $_.Current.Name -like $Pattern -and $_.Current.IsEnabled -and $_.Current.NativeWindowHandle -ne 0 -and $_.Current.Name.Length -lt 60 } | Select-Object -First 1
}

function Press($w, [string]$Pattern) {
  $e = Find $w $Pattern
  if (-not $e) { throw "not found: $Pattern" }
  [Dpi]::PostMessage([IntPtr]$e.Current.NativeWindowHandle, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
  Start-Sleep 1
}

function Untick($w, [string]$Pattern) {
  $e = Find $w $Pattern
  if (-not $e) { Write-Warning "checkbox not found: $Pattern"; return }
  $h = [IntPtr]$e.Current.NativeWindowHandle
  if ([Dpi]::SendMessage($h, 0x00F0, [IntPtr]::Zero, [IntPtr]::Zero) -eq 1) { [Dpi]::PostMessage($h, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null; Start-Sleep -Milliseconds 400 }
}

# The visible frame minus its 1-pixel edge, with the rounded corners painted over, so nothing behind the window shows.
function Shot($w, [string]$Name) {
  $r = New-Object Dpi+RECT
  [Dpi]::DwmGetWindowAttribute([IntPtr]$w.Current.NativeWindowHandle, 9, [ref]$r, 16) | Out-Null
  $bmp = New-Object System.Drawing.Bitmap ($r.R - $r.L - 2), ($r.B - $r.T - 2)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.L + 1, $r.T + 1, 0, 0, $bmp.Size)
  $c = $bmp.GetPixel(12, 2)
  for ($y = 0; $y -lt 9; $y++) { for ($x = 0; $x -lt 9; $x++) { if ([math]::Sqrt([math]::Pow(9 - $x, 2) + [math]::Pow(9 - $y, 2)) -gt 8) { foreach ($p in @(@($x, $y), @(($bmp.Width - 1 - $x), $y), @($x, ($bmp.Height - 1 - $y)), @(($bmp.Width - 1 - $x), ($bmp.Height - 1 - $y)))) { $bmp.SetPixel($p[0], $p[1], $c) } } } }
  $bmp.Save((Join-Path $Out $Name), [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
}

$p = Start-Process $Setup -PassThru
$w = Window $p
Start-Sleep 1
for ($i = 0; $i -lt 6 -and -not (Find $w '*when Windows starts*'); $i++) {
  Shot $w "installer-page$i.png"
  if (Find $w 'Install') { Press $w 'Install'; Start-Sleep 4 } elseif (Find $w 'Next*') { Press $w 'Next*' } else { Start-Sleep 1 }
}
Shot $w 'installer-finish.png'
$names = $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition) | ForEach-Object { $_.Current.Name } | Where-Object { $_ }
"finish page: " + ($names -join ' | ')
Untick $w '*when Windows starts*'
Untick $w 'Run *'
Untick $w '*desktop*'
[Dpi]::PostMessage([IntPtr]$w.Current.NativeWindowHandle, 0x0111, [IntPtr]1, [IntPtr](Find $w 'Finish').Current.NativeWindowHandle) | Out-Null
$p.WaitForExit(10000) | Out-Null
"installer exit code: $($p.ExitCode)"
