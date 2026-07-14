param(
    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path,
    [ValidateRange(0.01, 72.0)][double]$DurationHours = 24.0,
    [ValidateRange(1, 60)][int]$SampleIntervalSeconds = 30,
    [ValidateRange(0, 300)][int]$WarmupSeconds = 60,
    [ValidateSet("software")][string]$Renderer = "software"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
Import-Module (Join-Path $PSScriptRoot "m0-soak-lib.psm1") -Force

$Commit = $null
$Dirty = $false
Push-Location $RepositoryRoot
try {
    $Commit = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { throw "git rev-parse HEAD failed" }
    $DirtyEntries = @(& git status --porcelain)
    if ($LASTEXITCODE -ne 0) { throw "git status --porcelain failed" }
    $Dirty = $DirtyEntries.Count -gt 0
}
finally {
    Pop-Location
}
if ($DurationHours -ge 24.0 -and $Dirty) {
    throw "M0 soak requires one clean commit"
}

$Manifest = Join-Path $RepositoryRoot "Cargo.toml"
& cargo +1.97.0 build --manifest-path $Manifest -p tokenmaster-m0 --release --locked
if ($LASTEXITCODE -ne 0) { throw "release build failed" }

$ReportRoot = Join-Path $RepositoryRoot "reports"
New-Item -ItemType Directory -Path $ReportRoot -Force | Out-Null
$Stamp = [DateTimeOffset]::UtcNow.ToString("yyyyMMdd-HHmmss")
$CsvPath = Join-Path $ReportRoot "soak-$Stamp.csv"
$JsonPath = Join-Path $ReportRoot "soak-$Stamp.json"
$Executable = Join-Path $RepositoryRoot "target\x86_64-pc-windows-gnu\release\tokenmaster-m0.exe"
$ExecutableSha256 = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash.ToLowerInvariant()
$PreviousRenderer = $env:TOKENMASTER_RENDERER
$env:TOKENMASTER_RENDERER = $Renderer
$Process = $null
$Samples = [System.Collections.Generic.List[object]]::new()
$Started = $null
$Deadline = $null

function Add-ProcessSample {
    $Process.Refresh()
    if ($Process.HasExited) { throw "soak process exited early with code $($Process.ExitCode)" }
    $Gui = Get-ProcessGuiResources -Process $Process
    $Sample = [pscustomobject][ordered]@{
        utc = [DateTimeOffset]::UtcNow.ToString("O")
        privateBytes = $Process.PrivateMemorySize64
        workingSetBytes = $Process.WorkingSet64
        handles = $Process.HandleCount
        threads = $Process.Threads.Count
        userObjects = $Gui.userObjects
        gdiObjects = $Gui.gdiObjects
        cpuSeconds = $Process.TotalProcessorTime.TotalSeconds
    }
    $Samples.Add($Sample)
    Write-SoakCsvSample -Path $CsvPath -Sample $Sample
}

try {
    $Process = Start-Process -FilePath $Executable -PassThru -WindowStyle Hidden
    Start-Sleep -Seconds $WarmupSeconds
    $Started = [DateTimeOffset]::UtcNow
    $Deadline = $Started.AddHours($DurationHours)
    Add-ProcessSample
    while ([DateTimeOffset]::UtcNow -lt $Deadline) {
        Start-Sleep -Seconds $SampleIntervalSeconds
        Add-ProcessSample
    }
}
finally {
    if ($null -ne $Process -and -not $Process.HasExited) {
        Stop-Process -Id $Process.Id -Force
        $Process.WaitForExit()
    }
    if ($null -eq $PreviousRenderer) {
        Remove-Item Env:TOKENMASTER_RENDERER -ErrorAction SilentlyContinue
    } else {
        $env:TOKENMASTER_RENDERER = $PreviousRenderer
    }
}

$Completed = [DateTimeOffset]::UtcNow
$WallHours = ($Completed - $Started).TotalHours
$CompletedByWallClock = $WallHours -ge $DurationHours
$Evaluation = Get-SoakEvaluation -Samples $Samples.ToArray() -RequestedHours $DurationHours `
    -WallHours $WallHours -SampleIntervalSeconds $SampleIntervalSeconds `
    -ProcessorCount ([Environment]::ProcessorCount)
$Kind = if ($DurationHours -ge 24.0) { "m0-soak" } else { "developer-smoke" }
$Result = $Evaluation.result
$Summary = [ordered]@{
    schemaVersion = 1
    kind = $Kind
    result = $Result
    commit = $Commit
    dirty = $Dirty
    executableSha256 = $ExecutableSha256
    renderer = $Renderer
    warmupSeconds = $WarmupSeconds
    startedUtc = $Started.ToString("O")
    completedUtc = $Completed.ToString("O")
    os = [Environment]::OSVersion.VersionString
    processorCount = [Environment]::ProcessorCount
    requestedHours = $DurationHours
    wallHours = $WallHours
    completedByWallClock = $CompletedByWallClock
    sampledHours = $Evaluation.sampledHours
    sampleCount = $Samples.Count
    privateGrowthMiB = $Evaluation.privateGrowthMiB
    privateSlopeMiBPerHour = $Evaluation.privateSlopeMiBPerHour
    idleCpuPercent = $Evaluation.idleCpuPercent
    maxSampleGapSeconds = $Evaluation.maxSampleGapSeconds
    gapLimitSeconds = $Evaluation.gapLimitSeconds
    handleDelta = $Evaluation.handleDelta
    threadDelta = $Evaluation.threadDelta
    userObjectDelta = $Evaluation.userObjectDelta
    gdiObjectDelta = $Evaluation.gdiObjectDelta
    csv = [IO.Path]::GetFileName($CsvPath)
}
Write-AtomicJson -Path $JsonPath -Value $Summary
if ($DurationHours -ge 24.0 -and $Result -eq "pass") {
    $CanonicalPath = Join-Path $ReportRoot "soak-24h.json"
    Write-AtomicJson -Path $CanonicalPath -Value $Summary
}
if ($Result -ne "pass") { throw "soak gates failed" }
Write-Host "TokenMaster ${Kind}: PASS ($JsonPath)"
