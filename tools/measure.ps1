# Starts BrightPause, measures memory idle / in a break / after, saves screenshots of the app's own windows and tray menu, then quits.
param([Parameter(Mandatory)][string]$Exe, [string]$Out = "$PSScriptRoot\..\docs\shots", [switch]$Tray)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms, System.Drawing, UIAutomationClient, UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class W {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c, string n);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, uint d, UIntPtr e);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  public static void Click(int x, int y) { SetCursorPos(x, y); mouse_event(2, 0, 0, 0, UIntPtr.Zero); mouse_event(4, 0, 0, 0, UIntPtr.Zero); }
}
'@
[W]::SetProcessDPIAware() | Out-Null
$UIA = [System.Windows.Automation.AutomationElement]

function Tree-Ids([int]$RootId) {
  $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, Name, WorkingSetSize
  $ids = @($RootId); $added = $true
  while ($added) {
    $added = $false
    foreach ($p in $all) { if ($ids -contains $p.ParentProcessId -and $ids -notcontains $p.ProcessId) { $ids += $p.ProcessId; $added = $true } }
  }
  , @($all | Where-Object { $ids -contains $_.ProcessId })
}

function Tree-MB([int]$RootId) {
  $procs = Tree-Ids $RootId
  [pscustomobject]@{
    Processes = $procs.Count
    WebViews = @($procs | Where-Object Name -like 'msedgewebview2*').Count
    WorkingSetMB = [math]::Round((($procs | Measure-Object WorkingSetSize -Sum).Sum) / 1MB, 1)
    PrivateMB = [math]::Round(((Get-Process -Id $procs.ProcessId -ErrorAction SilentlyContinue | Measure-Object PrivateMemorySize64 -Sum).Sum) / 1MB, 1)
  }
}

function Shot([string]$Name, [System.Drawing.Rectangle]$R) {
  $bmp = New-Object System.Drawing.Bitmap $R.Width, $R.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($R.Left, $R.Top, 0, 0, $bmp.Size)
  $bmp.Save((Join-Path $Out $Name), [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
}

function Window-Rect([int]$ProcessId) {
  $cond = New-Object System.Windows.Automation.PropertyCondition($UIA::ProcessIdProperty, $ProcessId)
  $w = $UIA::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children, $cond)
  $b = $w.Current.BoundingRectangle
  New-Object System.Drawing.Rectangle ([int]$b.X), ([int]$b.Y), ([int]$b.Width), ([int]$b.Height)
}

function Find-Named($Root, [string]$Pattern) {
  $Root.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition) |
    Where-Object { $_.Current.Name -like $Pattern } | Select-Object -First 1
}

function Click-Element($E) {
  $b = $E.Current.BoundingRectangle
  [W]::Click([int]($b.X + $b.Width / 2), [int]($b.Y + $b.Height / 2))
}

function Tray-Shot([string]$Name) {
  $bar = $UIA::FromHandle([W]::FindWindow('Shell_TrayWnd', $null))
  $icon = Find-Named $bar 'BrightPause*'
  if (-not $icon) { Write-Warning 'tray icon not on the taskbar (pin it out of the overflow first)'; return }
  Click-Element $icon; Start-Sleep 1
  $menu = [W]::FindWindow('#32768', $null)
  $r = New-Object W+RECT
  if ($menu -eq [IntPtr]::Zero -or -not [W]::GetWindowRect($menu, [ref]$r)) { Write-Warning 'tray menu not found'; return }
  Shot $Name (New-Object System.Drawing.Rectangle ($r.L - 8), ($r.T - 8), ($r.R - $r.L + 16), ($r.B - $r.T + 16))
  [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
}

New-Item -ItemType Directory -Force $Out | Out-Null
$p = Start-Process $Exe -PassThru
Start-Sleep 20
$idle = Tree-MB $p.Id
if ($Tray) { Tray-Shot 'real-tray.png' }
& $Exe --break-now; Start-Sleep 6
$inBreak = Tree-MB $p.Id
Shot 'real-break.png' ([System.Windows.Forms.Screen]::PrimaryScreen.Bounds)
& $Exe --skip; Start-Sleep 15
$after = Tree-MB $p.Id
& $Exe --settings; Start-Sleep 6
$settings = Tree-MB $p.Id
Shot 'real-settings.png' (Window-Rect $p.Id)
& $Exe --quit; Start-Sleep 3
@(
  [pscustomobject]@{ Stage = 'idle, tray only'; Data = $idle }
  [pscustomobject]@{ Stage = 'during a break'; Data = $inBreak }
  [pscustomobject]@{ Stage = '15 s after the break'; Data = $after }
  [pscustomobject]@{ Stage = 'settings open'; Data = $settings }
) | ForEach-Object { "{0,-22} processes {1,2}  webviews {2,2}  working set {3,6} MB  private {4,6} MB" -f $_.Stage, $_.Data.Processes, $_.Data.WebViews, $_.Data.WorkingSetMB, $_.Data.PrivateMB }
"screens: $([System.Windows.Forms.Screen]::AllScreens.Count)"
"still running after quit: $([bool](Get-Process -Id $p.Id -ErrorAction SilentlyContinue))"
