# Measures what the product costs while it ingests: archive growth, write-ahead-log growth,
# and whole-process processor time, sampled against the live archive.
#
# The journal is the sensitive number. Before the ingestion fixes it grew about 16 MB every
# ten seconds while the database itself gained 30 KB in two minutes; afterwards it does not
# grow across any sample. A regression shows here long before it shows in a stopwatch.
#
# Operator-run against a built binary. Nothing invokes it automatically.
param(
    [string]$Exe = "C:\code\tokenmaster\target\x86_64-pc-windows-msvc\release\TokenMaster.exe",
    [int]$Seconds = 120,
    [int]$Every = 5
)

$db  = "$env:LOCALAPPDATA\tokenmaster\tokenmaster.sqlite3"
$wal = "$db-wal"

Get-Process -Name TokenMaster -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 800

function MB([string]$path) {
    if (Test-Path $path) { [math]::Round((Get-Item $path).Length / 1MB, 2) } else { 0 }
}

"# seconds  db_MB  wal_MB  cpu_percent_of_one_core"
"before launch: db $(MB $db) MB, wal $(MB $wal) MB"

$process = Start-Process -FilePath $Exe -PassThru
Start-Sleep -Milliseconds 500
$previousCpu = 0.0
$walSamples = @()

for ($t = $Every; $t -le $Seconds; $t += $Every) {
    Start-Sleep -Seconds $Every
    $live = Get-Process -Id $process.Id -ErrorAction SilentlyContinue
    if (-not $live) { "$t`tprocess exited"; break }
    $cpu = $live.TotalProcessorTime.TotalSeconds
    $percent = [math]::Round(($cpu - $previousCpu) / $Every * 100, 0)
    $previousCpu = $cpu
    $walNow = MB $wal
    $walSamples += $walNow
    "$t`t$(MB $db)`t$walNow`t$percent"
}

$live = Get-Process -Id $process.Id -ErrorAction SilentlyContinue
if ($live) {
    "total processor seconds: $([math]::Round($live.TotalProcessorTime.TotalSeconds,1))"
    "working set MB: $([math]::Round($live.WorkingSet64/1MB,1))"
    $live | Stop-Process -Force
}

# Journal growth between consecutive samples, ignoring the drops where a checkpoint reset it.
$growth = @()
for ($i = 1; $i -lt $walSamples.Count; $i++) {
    $delta = $walSamples[$i] - $walSamples[$i - 1]
    if ($delta -gt 0) { $growth += $delta }
}
if ($growth.Count -gt 0) {
    $sum = ($growth | Measure-Object -Sum).Sum
    "journal growth per $Every s: max $([math]::Round(($growth | Measure-Object -Maximum).Maximum,2)) MB, total $([math]::Round($sum,2)) MB over $Seconds s"
} else {
    "journal did not grow across any sample"
}
