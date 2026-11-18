# Runs the app through idle, a break and the settings window while sampling every TCP connection
# and UDP endpoint owned by its process tree (every 250 ms). Prints anything that is not loopback.
param([Parameter(Mandatory)][string]$Exe, [int]$IdleSeconds = 60)
$ErrorActionPreference = 'Stop'

function Tree-Ids([int]$RootId) {
  $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, Name, CommandLine
  $ids = @($RootId); $added = $true
  while ($added) {
    $added = $false
    foreach ($p in $all) { if ($ids -contains $p.ParentProcessId -and $ids -notcontains $p.ProcessId) { $ids += $p.ProcessId; $added = $true } }
  }
  $ids
}

$seen = @{}
$samples = 0
function Sample([int]$RootId, [double]$Seconds) {
  $until = (Get-Date).AddSeconds($Seconds)
  while ((Get-Date) -lt $until) {
    $ids = Tree-Ids $RootId
    foreach ($c in Get-NetTCPConnection -ErrorAction SilentlyContinue | Where-Object { $ids -contains $_.OwningProcess }) {
      $proc = Get-CimInstance Win32_Process -Filter "ProcessId=$($c.OwningProcess)"
      $seen["tcp $($c.RemoteAddress):$($c.RemotePort) $($c.State) pid $($c.OwningProcess) $($proc.Name) parent $($proc.ParentProcessId)"] = $true
    }
    foreach ($u in Get-NetUDPEndpoint -ErrorAction SilentlyContinue | Where-Object { $ids -contains $_.OwningProcess }) {
      $seen["udp local $($u.LocalAddress):$($u.LocalPort)"] = $true
    }
    $script:samples++
    Start-Sleep -Milliseconds 250
  }
}

$p = Start-Process $Exe -PassThru
Sample $p.Id $IdleSeconds
& $Exe --break-now; Sample $p.Id 15
& $Exe --skip; Sample $p.Id 5
& $Exe --settings; Sample $p.Id 15
& $Exe --quit; Start-Sleep 2
"samples: $samples"
$outside = $seen.Keys | Where-Object { $_ -notmatch '(127\.0\.0\.1|::1|0\.0\.0\.0|::):' }
"endpoints seen: $($seen.Count)"
$seen.Keys | Sort-Object
"not loopback: $(@($outside).Count)"
