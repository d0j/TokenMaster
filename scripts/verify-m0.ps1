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

function Write-M0StageBegin {
    param([string]$Id)
    Write-Host "TM-M0-STAGE-BEGIN $Id"
}

function Write-M0StagePass {
    param([string]$Id)
    Write-Host "TM-M0-STAGE-PASS $Id"
}

Write-M0StageBegin "clean-root"
& (Join-Path $PSScriptRoot "audit-clean-root.ps1") -RepositoryRoot $RepositoryRoot
Write-M0StagePass "clean-root"
Write-M0StageBegin "immutable-actions"
& (Join-Path $PSScriptRoot "validate-immutable-actions.ps1") `
    -RepositoryRoot $RepositoryRoot
if ($LASTEXITCODE -ne 0) {
    throw "GitHub Actions references are not immutable"
}
Write-M0StagePass "immutable-actions"
Write-M0StageBegin "dependency-policy"
& (Join-Path $PSScriptRoot "verify-dependency-policy.ps1") `
    -RepositoryRoot $RepositoryRoot
if ($LASTEXITCODE -ne 0) {
    throw "Dependency policy did not pass"
}
Write-M0StagePass "dependency-policy"

$Cargo = (Get-Command cargo.exe -CommandType Application -ErrorAction Stop | Select-Object -First 1).Name

function Invoke-Checked {
    param([string]$Id, [string]$File, [string[]]$Arguments)
    Write-M0StageBegin $Id
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
    Write-M0StagePass $Id
}

function Invoke-PesterChecked {
    param([string]$Id, [string]$Path)
    Write-M0StageBegin $Id
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
    Write-M0StagePass $Id
}

Invoke-PesterChecked "pester-m0-scripts" (Join-Path $PSScriptRoot "tests\m0-scripts.Tests.ps1")
Invoke-PesterChecked "pester-immutable-actions" (Join-Path $PSScriptRoot "tests\immutable-actions.Tests.ps1")
Invoke-PesterChecked "pester-release-artifact-workflow" (Join-Path $PSScriptRoot "tests\release-artifact-workflow.Tests.ps1")
Invoke-PesterChecked "pester-dependency-policy" (Join-Path $PSScriptRoot "tests\dependency-policy.Tests.ps1")
Invoke-PesterChecked "pester-secret-scan" (Join-Path $PSScriptRoot "tests\secret-scan.Tests.ps1")
Invoke-Checked "fmt" $Cargo @("fmt", "--manifest-path", $Manifest, "--all", "--", "--check")
$PreviousRustFlags = $env:RUSTFLAGS
try {
    $env:RUSTFLAGS = "$PreviousRustFlags -Dwarnings".Trim()
    Invoke-Checked "clippy" $Cargo @("clippy", "--manifest-path", $Manifest, "--workspace", "--all-targets", "--locked")
}
finally {
    if ($null -eq $PreviousRustFlags) {
        Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
    } else {
        $env:RUSTFLAGS = $PreviousRustFlags
    }
}
Invoke-Checked "sqlite-one-million" $Cargo @("test", "--manifest-path", $Manifest, "-p", "tokenmaster-store", "--test", "sqlite_contract", "--locked", "--", "one_million_rows_remain_page_bounded", "--ignored", "--exact")
Invoke-Checked "workspace-tests" $Cargo @("test", "--manifest-path", $Manifest, "--workspace", "--locked")
Invoke-Checked "product-release-build" $Cargo @("build", "--manifest-path", $Manifest, "-p", "tokenmaster-app", "--release", "--locked")

$Summary = [ordered]@{
    schemaVersion = 1
    kind = "developer-verification"
    result = "pass"
    toolchain = "rust-1.97"
    target = "x86_64-pc-windows-msvc"
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
