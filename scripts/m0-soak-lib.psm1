Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Write-SoakCsvSample {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Sample
    )

    $Append = Test-Path -LiteralPath $Path
    $Sample | Export-Csv -LiteralPath $Path -NoTypeInformation -Append:$Append -Encoding utf8
}

function Write-AtomicJson {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)]$Value
    )

    $TemporaryPath = "$Path.tmp"
    $Json = $Value | ConvertTo-Json -Depth 8
    $Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [IO.File]::WriteAllText($TemporaryPath, $Json, $Utf8NoBom)
    Move-Item -LiteralPath $TemporaryPath -Destination $Path -Force
}

function Get-PrivateSlopeMiBPerHour {
    param([Parameter(Mandatory = $true)][object[]]$Samples)

    $Origin = [DateTimeOffset]::Parse($Samples[0].utc)
    $Count = [double]$Samples.Count
    $SumX = 0.0
    $SumY = 0.0
    $SumXY = 0.0
    $SumXX = 0.0
    foreach ($Sample in $Samples) {
        $X = ([DateTimeOffset]::Parse($Sample.utc) - $Origin).TotalHours
        $Y = [double]$Sample.privateBytes / 1MB
        $SumX += $X
        $SumY += $Y
        $SumXY += $X * $Y
        $SumXX += $X * $X
    }
    $Denominator = ($Count * $SumXX) - ($SumX * $SumX)
    if ($Denominator -le 0.0) { return [double]::PositiveInfinity }
    return (($Count * $SumXY) - ($SumX * $SumY)) / $Denominator
}

function Get-ProcessGuiResources {
    param([Parameter(Mandatory = $true)][System.Diagnostics.Process]$Process)

    if (-not ("TokenMaster.NativeGuiResources" -as [type])) {
        Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
namespace TokenMaster {
    public static class NativeGuiResources {
        [DllImport("user32.dll", SetLastError = true)]
        public static extern uint GetGuiResources(IntPtr process, uint flags);
    }
}
"@
    }

    [pscustomobject]@{
        userObjects = [TokenMaster.NativeGuiResources]::GetGuiResources($Process.Handle, 1)
        gdiObjects = [TokenMaster.NativeGuiResources]::GetGuiResources($Process.Handle, 0)
    }
}

function Get-SoakEvaluation {
    param(
        [Parameter(Mandatory = $true)][object[]]$Samples,
        [Parameter(Mandatory = $true)][double]$RequestedHours,
        [Parameter(Mandatory = $true)][double]$WallHours,
        [Parameter(Mandatory = $true)][int]$SampleIntervalSeconds,
        [Parameter(Mandatory = $true)][int]$ProcessorCount
    )

    if ($Samples.Count -lt 2) { throw "soak requires at least two samples" }
    if ($ProcessorCount -lt 1) { throw "processor count must be positive" }

    $First = $Samples[0]
    $Last = $Samples[$Samples.Count - 1]
    $FirstUtc = [DateTimeOffset]::Parse($First.utc)
    $LastUtc = [DateTimeOffset]::Parse($Last.utc)
    $SampledHours = ($LastUtc - $FirstUtc).TotalHours
    $PrivateGrowthMiB = ([double]$Last.privateBytes - [double]$First.privateBytes) / 1MB
    $PrivateSlopeMiBPerHour = Get-PrivateSlopeMiBPerHour -Samples $Samples
    $IdleCpuPercent = if ($SampledHours -gt 0.0) {
        (([double]$Last.cpuSeconds - [double]$First.cpuSeconds) / ($SampledHours * 3600.0)) * 100.0 / $ProcessorCount
    } else {
        [double]::PositiveInfinity
    }

    $MaxSampleGapSeconds = 0.0
    for ($Index = 1; $Index -lt $Samples.Count; $Index++) {
        $PreviousUtc = [DateTimeOffset]::Parse($Samples[$Index - 1].utc)
        $CurrentUtc = [DateTimeOffset]::Parse($Samples[$Index].utc)
        $Gap = ($CurrentUtc - $PreviousUtc).TotalSeconds
        if ($Gap -gt $MaxSampleGapSeconds) { $MaxSampleGapSeconds = $Gap }
    }

    $HandleDelta = [int]$Last.handles - [int]$First.handles
    $ThreadDelta = [int]$Last.threads - [int]$First.threads
    $UserObjectDelta = [int]$Last.userObjects - [int]$First.userObjects
    $GdiObjectDelta = [int]$Last.gdiObjects - [int]$First.gdiObjects
    $Completed = $WallHours -ge $RequestedHours
    $GapLimitSeconds = [double]$SampleIntervalSeconds * 2.5
    $Pass = $Completed -and
        $PrivateGrowthMiB -le 8.0 -and
        $PrivateSlopeMiBPerHour -le 0.25 -and
        $IdleCpuPercent -lt 0.2 -and
        $MaxSampleGapSeconds -le $GapLimitSeconds -and
        $HandleDelta -le 0 -and
        $ThreadDelta -le 0 -and
        $UserObjectDelta -le 0 -and
        $GdiObjectDelta -le 0

    [pscustomobject][ordered]@{
        result = if ($Pass) { "pass" } else { "fail" }
        sampledHours = $SampledHours
        privateGrowthMiB = $PrivateGrowthMiB
        privateSlopeMiBPerHour = $PrivateSlopeMiBPerHour
        idleCpuPercent = $IdleCpuPercent
        maxSampleGapSeconds = $MaxSampleGapSeconds
        gapLimitSeconds = $GapLimitSeconds
        handleDelta = $HandleDelta
        threadDelta = $ThreadDelta
        userObjectDelta = $UserObjectDelta
        gdiObjectDelta = $GdiObjectDelta
    }
}

Export-ModuleMember -Function Write-SoakCsvSample, Write-AtomicJson, Get-ProcessGuiResources, Get-SoakEvaluation
