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
    # A suite that ran nothing is not a suite that passed, and neither is one that skipped
    # everything. Measured on Pester 5.7.1 against a fixture whose only `It` is commented out:
    # FailedCount 0, PassedCount 0, TotalCount 0 and Result "Passed" -- so FailedCount alone
    # calls an empty stage green, and any suite that quietly stopped testing would leave every
    # stage-count claim downstream meaning one fewer. `PassedCount -gt 0` alone is not enough
    # either: `-Skip` on all but one test leaves that one passing. Every suite runs between 2
    # and 31 tests and skips none, so both conditions cost nothing today.
    $ExitCode = if ($Result.FailedCount -eq 0 -and $Result.SkippedCount -eq 0 `
            -and $Result.PassedCount -gt 0) { 0 } else { 1 }
    # The stage's own id, not the constant "pester" every one of them used to record: the
    # receipt could not distinguish seven Pester stages from eight, which is the same blindness
    # the composition guard had.
    $Commands.Add([ordered]@{
        id = $Id
        startedUtc = $Started.ToString("O")
        exitCode = $ExitCode
        passed = $Result.PassedCount
        skipped = $Result.SkippedCount
    })
    if ($ExitCode -ne 0) {
        throw ("Invoke-Pester failed for {0}: {1} passed, {2} failed, {3} skipped, {4} total" -f `
                $Path, $Result.PassedCount, $Result.FailedCount, $Result.SkippedCount, `
                $Result.TotalCount)
    }
    Write-M0StagePass $Id
}

# Enumerated from disk rather than listed, because the list is what failed. The guard that
# pinned this composition asserted each suite's file name appeared somewhere in this script's
# text -- so putting a `#` in front of a stage left the name inside that very comment, the
# guard green, and the gate quietly running seven suites while printing fifteen stages. A
# suite that exists is now run by construction, and a ninth added tomorrow needs nobody to
# remember it. The floor is a separate guard: an enumeration that finds nothing must not pass
# as a gate that ran everything.
$PesterSuites = @(Get-ChildItem -LiteralPath (Join-Path $PSScriptRoot "tests") `
        -Filter "*.Tests.ps1" -File | Sort-Object -Property Name)
if ($PesterSuites.Count -lt 8) {
    throw "expected at least eight Pester suites, found $($PesterSuites.Count)"
}
foreach ($Suite in $PesterSuites) {
    Invoke-PesterChecked ("pester-" + ($Suite.BaseName -replace '\.Tests$', '')) $Suite.FullName
}
# Checked against the run's own record, not against this file's text. The composition guard in
# `m0-scripts.Tests.ps1` reads source, and source can be commented out: a `#` in front of the
# enumeration above leaves every suite name inside the comment where a regex still finds it,
# and the gate would run zero Pester suites with the contract green. That is the third
# appearance of one shape, and reading text is what all three had in common. `$Commands` is
# what the receipt is built from, so a suite with no entry here did not run whatever the
# script says -- and with `Set-StrictMode -Version Latest`, deleting the enumeration makes
# `$PesterSuites` an error rather than an empty expectation.
$RecordedPesterStages = @(
    $Commands | Where-Object { $_.id -like "pester-*" -and $_.exitCode -eq 0 } |
        ForEach-Object { $_.id }
)
$MissingPesterStages = @(
    $PesterSuites |
        ForEach-Object { "pester-" + ($_.BaseName -replace '\.Tests$', '') } |
        Where-Object { $RecordedPesterStages -notcontains $_ }
)
if ($MissingPesterStages.Count -gt 0) {
    throw "the run recorded no passing stage for: $($MissingPesterStages -join ', ')"
}
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
