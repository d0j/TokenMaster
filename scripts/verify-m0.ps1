param(
    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$Manifest = Join-Path $RepositoryRoot "Cargo.toml"
$ReportRoot = Join-Path $RepositoryRoot "reports"
$Commands = [System.Collections.Generic.List[object]]::new()
New-Item -ItemType Directory -Path $ReportRoot -Force | Out-Null

$RequiredPesterVersion = [version]"5.7.1"
$PesterModule = Get-Module -ListAvailable Pester |
    Where-Object { $_.Version -eq $RequiredPesterVersion } |
    Select-Object -First 1
if (-not $PesterModule) {
    throw "Pester $RequiredPesterVersion is required. Install-Module Pester -RequiredVersion $RequiredPesterVersion -Scope CurrentUser"
}
Import-Module Pester -RequiredVersion $RequiredPesterVersion -Force
& (Join-Path $PSScriptRoot "audit-clean-root.ps1") -RepositoryRoot $RepositoryRoot

$MingwRoot = $null
$MingwLinker = $null
foreach ($CandidateRoot in @("C:\mingw64", "C:\msys64\mingw64")) {
    $CandidateLinker = Join-Path $CandidateRoot "bin\x86_64-w64-mingw32-gcc.exe"
    $CandidateImportLibraries = @(
        (Join-Path $CandidateRoot "lib\libshlwapi.a"),
        (Join-Path $CandidateRoot "x86_64-w64-mingw32\lib\libshlwapi.a")
    )
    $CandidateShlwapi = $CandidateImportLibraries |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
        Select-Object -First 1
    if ((Test-Path -LiteralPath $CandidateLinker -PathType Leaf) -and $CandidateShlwapi) {
        $MingwRoot = $CandidateRoot
        $MingwLinker = $CandidateLinker
        break
    }
}
if (-not $MingwRoot) {
    throw "MinGW with the shlwapi import library is required under C:\mingw64 or C:\msys64\mingw64"
}
$MingwBin = Join-Path $MingwRoot "bin"
$env:Path = "$MingwBin$([IO.Path]::PathSeparator)${env:Path}"
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = $MingwLinker
$MingwVersionOutput = @(& $MingwLinker --version)
$MingwExitCode = $LASTEXITCODE
$MingwVersion = $MingwVersionOutput | Select-Object -First 1
if ($MingwExitCode -ne 0 -or [string]::IsNullOrWhiteSpace($MingwVersion)) {
    throw "The validated GNU linker did not report a version"
}

$Cargo = (Get-Command cargo.exe -CommandType Application -ErrorAction Stop | Select-Object -First 1).Name

function Invoke-Checked {
    param([string]$File, [string[]]$Arguments)
    $Started = [DateTimeOffset]::UtcNow
    & $File @Arguments
    $ExitCode = $LASTEXITCODE
    $Commands.Add([ordered]@{
        id = [IO.Path]::GetFileNameWithoutExtension($File)
        startedUtc = $Started.ToString("O")
        exitCode = $ExitCode
    })
    if ($ExitCode -ne 0) {
        throw "$File failed with exit code $ExitCode"
    }
}

function Invoke-PesterChecked {
    param([string]$Path)
    $Started = [DateTimeOffset]::UtcNow
    $Result = Invoke-Pester $Path -PassThru
    $ExitCode = if ($Result.FailedCount -eq 0) { 0 } else { 1 }
    $Commands.Add([ordered]@{
        id = "pester"
        startedUtc = $Started.ToString("O")
        exitCode = $ExitCode
    })
    if ($ExitCode -ne 0) {
        throw "Invoke-Pester failed for $Path"
    }
}

$Stamp = [DateTimeOffset]::UtcNow.ToString("yyyyMMdd-HHmmssfff")
Invoke-PesterChecked (Join-Path $PSScriptRoot "tests\m0-soak-lib.Tests.ps1")
Invoke-PesterChecked (Join-Path $PSScriptRoot "tests\m0-scripts.Tests.ps1")
Invoke-Checked $Cargo @("+1.97.0", "fmt", "--manifest-path", $Manifest, "--all", "--", "--check")
$PreviousRustFlags = $env:RUSTFLAGS
try {
    $env:RUSTFLAGS = "$PreviousRustFlags -Dwarnings".Trim()
    Invoke-Checked $Cargo @("+1.97.0", "clippy", "--manifest-path", $Manifest, "--workspace", "--all-targets", "--locked")
}
finally {
    if ($null -eq $PreviousRustFlags) {
        Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
    } else {
        $env:RUSTFLAGS = $PreviousRustFlags
    }
}
Invoke-Checked $Cargo @("+1.97.0", "test", "--manifest-path", $Manifest, "-p", "tokenmaster-gates", "--test", "budget_contract", "--locked")
Invoke-Checked $Cargo @("+1.97.0", "test", "--manifest-path", $Manifest, "-p", "tokenmaster-m0", "--test", "metrics_contract", "--locked")
Invoke-Checked $Cargo @("+1.97.0", "test", "--manifest-path", $Manifest, "-p", "tokenmaster-m0", "--test", "stress_contract", "--locked")
Invoke-Checked $Cargo @("+1.97.0", "test", "--manifest-path", $Manifest, "-p", "tokenmaster-store", "--test", "sqlite_contract", "--locked", "--", "one_million_rows_remain_page_bounded", "--ignored", "--exact")
Invoke-Checked $Cargo @("+1.97.0", "test", "--manifest-path", $Manifest, "--workspace", "--locked")
Invoke-Checked $Cargo @("+1.97.0", "build", "--manifest-path", $Manifest, "-p", "tokenmaster-m0", "--release", "--locked")

$Executable = Join-Path $RepositoryRoot "target\x86_64-pc-windows-gnu\release\tokenmaster-m0.exe"
Invoke-Checked $Executable @("--stress", "switches", "--iterations", "100", "--duration-seconds", "2", "--rows", "1000", "--report", "reports/verify-switches-$Stamp.json")
Invoke-Checked $Executable @("--stress", "routes", "--iterations", "100", "--duration-seconds", "2", "--rows", "1000", "--report", "reports/verify-routes-$Stamp.json")

$Summary = [ordered]@{
    schemaVersion = 1
    kind = "developer-verification"
    result = "pass"
    toolchain = "rust-1.97"
    mingw = "validated"
    pester = "5.7.1"
    commands = $Commands
    externalGates = @(
        "interactive Windows tray/DPI/accessibility matrix is unverified",
        "24-hour soak is unverified"
    )
}
$Destination = Join-Path $ReportRoot "verification-summary.json"
$Temporary = "$Destination.tmp"
$Summary | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $Temporary -Encoding utf8NoBOM
Move-Item -LiteralPath $Temporary -Destination $Destination -Force
Write-Host "TokenMaster M0 automated verification: PASS"
Write-Host "External interactive and 24-hour gates: UNVERIFIED"
