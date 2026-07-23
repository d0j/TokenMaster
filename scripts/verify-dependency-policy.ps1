param(
    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "release-tooling.ps1")

$Manifest = Join-Path $RepositoryRoot "Cargo.toml"
$LockFile = Join-Path $RepositoryRoot "Cargo.lock"
$Policy = Join-Path $RepositoryRoot "deny.toml"
$ReportRoot = Join-Path $RepositoryRoot "reports"
$ScratchRoot = Join-Path $RepositoryRoot "target\dependency-policy"
$MetadataPath = Join-Path $ScratchRoot "metadata.json"
$CargoHome = Join-Path $ScratchRoot "cargo-home"
$Checks = @("advisories", "licenses", "sources")

foreach ($RequiredFile in @($Manifest, $LockFile, $Policy)) {
    if (-not (Test-Path -LiteralPath $RequiredFile -PathType Leaf)) {
        throw "dependency policy input is missing"
    }
}

$CargoDeny = (& (Join-Path $PSScriptRoot "install-cargo-deny.ps1") `
        -RepositoryRoot $RepositoryRoot | Select-Object -Last 1)
if (-not (Test-Path -LiteralPath $CargoDeny -PathType Leaf)) {
    throw "cargo-deny bootstrap did not return an executable"
}
$ToolVersion = (@(& $CargoDeny --version) -join "").Trim()
if ($LASTEXITCODE -ne 0 -or $ToolVersion -ne "cargo-deny 0.20.2") {
    throw "cargo-deny version does not match the reviewed release"
}
$StateBefore = Get-DependencyPolicyState `
    -RepositoryRoot $RepositoryRoot `
    -CargoDenyPath $CargoDeny `
    -PolicyPath $Policy `
    -LockPath $LockFile

if (Test-Path -LiteralPath $ScratchRoot) {
    Remove-TaskDirectory -Path $ScratchRoot -AllowedRoot (Join-Path $RepositoryRoot "target")
}
New-Item -ItemType Directory -Path $ScratchRoot -Force | Out-Null

$Cargo = (Get-Command cargo.exe -CommandType Application -ErrorAction Stop |
        Select-Object -First 1).Source
$Metadata = @(& $Cargo "+1.97.0" "metadata" "--manifest-path" $Manifest `
        "--format-version" "1" "--locked" "--all-features")
if ($LASTEXITCODE -ne 0) {
    throw "locked dependency metadata failed"
}
[IO.File]::WriteAllText($MetadataPath, ($Metadata -join [Environment]::NewLine))

$PreviousCargoHome = $env:CARGO_HOME
try {
    $env:CARGO_HOME = $CargoHome
    & $CargoDeny "--manifest-path" $Manifest "--metadata-path" $MetadataPath `
        "--config" $Policy "--workspace" "--locked" "--all-features" `
        "check" @Checks
    if ($LASTEXITCODE -ne 0) {
        throw "dependency policy failed"
    }
}
finally {
    if ($null -eq $PreviousCargoHome) {
        Remove-Item Env:CARGO_HOME -ErrorAction SilentlyContinue
    } else {
        $env:CARGO_HOME = $PreviousCargoHome
    }
    if (Test-Path -LiteralPath $ScratchRoot) {
        Remove-TaskDirectory -Path $ScratchRoot -AllowedRoot (Join-Path $RepositoryRoot "target")
    }
}

$StateAfter = Get-DependencyPolicyState `
    -RepositoryRoot $RepositoryRoot `
    -CargoDenyPath $CargoDeny `
    -PolicyPath $Policy `
    -LockPath $LockFile
Assert-DependencyPolicyStateUnchanged -Before $StateBefore -After $StateAfter
New-Item -ItemType Directory -Path $ReportRoot -Force | Out-Null
$Receipt = [ordered]@{
    schemaVersion = 1
    kind = "dependency-policy"
    result = "pass"
    commit = $StateBefore.Commit
    dirty = $StateBefore.Dirty
    tool = "cargo-deny"
    toolVersion = "0.20.2"
    toolSha256 = $StateBefore.ToolSha256
    policySha256 = $StateBefore.PolicySha256
    lockSha256 = $StateBefore.LockSha256
    target = "x86_64-pc-windows-msvc"
    checks = $Checks
}
$Destination = Join-Path $ReportRoot "dependency-policy.json"
$Temporary = "$Destination.tmp"
$Receipt | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $Temporary -Encoding utf8NoBOM
Move-Item -LiteralPath $Temporary -Destination $Destination -Force
Write-Output "dependency-policy-pass"
