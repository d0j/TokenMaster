Describe "TokenMaster M0 soak helpers" {
    BeforeAll {
        $ModulePath = Join-Path (Split-Path -Parent $PSScriptRoot) "m0-soak-lib.psm1"
        Import-Module $ModulePath -Force

        function New-TestSample {
            param(
                [string]$Utc,
                [long]$PrivateBytes = 16MB,
                [double]$CpuSeconds = 0.0,
                [int]$Handles = 10,
                [int]$Threads = 4,
                [int]$UserObjects = 3,
                [int]$GdiObjects = 1
            )

            [pscustomobject]@{
                utc = $Utc
                privateBytes = $PrivateBytes
                workingSetBytes = 20MB
                handles = $Handles
                threads = $Threads
                userObjects = $UserObjects
                gdiObjects = $GdiObjects
                cpuSeconds = $CpuSeconds
            }
        }
    }

    It "appends one durable CSV row without replacing prior samples" {
        $Path = Join-Path $TestDrive "samples.csv"
        Write-SoakCsvSample -Path $Path -Sample (New-TestSample -Utc "2026-07-13T00:00:00Z")
        Write-SoakCsvSample -Path $Path -Sample (New-TestSample -Utc "2026-07-13T00:00:30Z")

        $Rows = @(Import-Csv -LiteralPath $Path)
        $Rows.Count | Should -Be 2
        $Rows[0].utc | Should -Be "2026-07-13T00:00:00Z"
        $Rows[1].utc | Should -Be "2026-07-13T00:00:30Z"
    }

    It "replaces JSON atomically and leaves no temporary file" {
        $Path = Join-Path $TestDrive "summary.json"
        Set-Content -LiteralPath $Path -Value '{"result":"old"}' -Encoding utf8

        Write-AtomicJson -Path $Path -Value ([ordered]@{ result = "pass" })

        (Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json).result | Should -Be "pass"
        (Test-Path -LiteralPath "$Path.tmp") | Should -Be $false
    }

    It "passes only a complete stable bounded sample series" {
        $Samples = @(
            New-TestSample -Utc "2026-07-13T00:00:00Z" -CpuSeconds 0.000
            New-TestSample -Utc "2026-07-13T00:00:30Z" -CpuSeconds 0.001
            New-TestSample -Utc "2026-07-13T00:01:00Z" -CpuSeconds 0.002
        )

        $Result = Get-SoakEvaluation -Samples $Samples -RequestedHours 0.01 `
            -WallHours 0.011 -SampleIntervalSeconds 30 -ProcessorCount 8

        $Result.result | Should -Be "pass"
        $Result.privateSlopeMiBPerHour | Should -Be 0
        $Result.maxSampleGapSeconds | Should -Be 30
        $Result.handleDelta | Should -Be 0
        $Result.threadDelta | Should -Be 0
        $Result.userObjectDelta | Should -Be 0
        $Result.gdiObjectDelta | Should -Be 0
    }

    It "fails sustained memory growth" {
        $Samples = @(
            New-TestSample -Utc "2026-07-13T00:00:00Z" -PrivateBytes 16MB
            New-TestSample -Utc "2026-07-13T00:00:30Z" -PrivateBytes 17MB
            New-TestSample -Utc "2026-07-13T00:01:00Z" -PrivateBytes 18MB
        )

        $Result = Get-SoakEvaluation -Samples $Samples -RequestedHours 0.01 `
            -WallHours 0.011 -SampleIntervalSeconds 30 -ProcessorCount 8

        $Result.result | Should -Be "fail"
        ($Result.privateSlopeMiBPerHour -gt 0.25) | Should -Be $true
    }

    It "fails sleep gaps and counter drift instead of interpolating" {
        $Samples = @(
            New-TestSample -Utc "2026-07-13T00:00:00Z"
            New-TestSample -Utc "2026-07-13T00:02:00Z" -Handles 11 -Threads 5 `
                -UserObjects 4 -GdiObjects 2
        )

        $Result = Get-SoakEvaluation -Samples $Samples -RequestedHours 0.01 `
            -WallHours 0.011 -SampleIntervalSeconds 30 -ProcessorCount 8

        $Result.result | Should -Be "fail"
        $Result.maxSampleGapSeconds | Should -Be 120
        $Result.handleDelta | Should -Be 1
        $Result.threadDelta | Should -Be 1
        $Result.userObjectDelta | Should -Be 1
        $Result.gdiObjectDelta | Should -Be 1
    }
}
