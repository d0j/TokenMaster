param(
    [Parameter(Mandatory)]
    [string]$PackagePath,

    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "release-tooling.ps1")

$Repository = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $RepositoryRoot).Path)
$Package = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $PackagePath).Path)
$ReportRoot = Join-Path $Repository "reports"
$ScratchRoot = Join-Path $Repository "target\secret-scan"
$SourceReport = Join-Path $ScratchRoot "source.json"
$PackageReport = Join-Path $ScratchRoot "package.json"
$Config = Join-Path $Repository ".gitleaks.toml"
$Ignore = Join-Path $Repository ".gitleaksignore"
foreach ($PolicyFile in @($Config, $Ignore)) {
    if (-not (Test-Path -LiteralPath $PolicyFile -PathType Leaf)) {
        throw "secret scan policy input is missing"
    }
}

function Get-SecretScanState {
    $Git = (Get-Command git.exe -CommandType Application -ErrorAction Stop |
            Select-Object -First 1).Source
    $Commit = (@(& $Git -C $Repository rev-parse HEAD) -join "").Trim()
    if ($LASTEXITCODE -ne 0 -or $Commit -notmatch '^[0-9a-f]{40}$') {
        throw "secret scan could not bind the current commit"
    }
    $Status = @(& $Git -C $Repository status --porcelain)
    if ($LASTEXITCODE -ne 0) {
        throw "secret scan could not bind the worktree state"
    }
    [pscustomobject]@{
        Commit = $Commit
        Dirty = $Status.Count -ne 0
        ToolSha256 = (Get-FileHash -LiteralPath $Gitleaks -Algorithm SHA256).
            Hash.ToLowerInvariant()
        PackageSha256 = (Get-FileHash -LiteralPath $Package -Algorithm SHA256).
            Hash.ToLowerInvariant()
        ConfigSha256 = (Get-FileHash -LiteralPath $Config -Algorithm SHA256).
            Hash.ToLowerInvariant()
        IgnoreSha256 = (Get-FileHash -LiteralPath $Ignore -Algorithm SHA256).
            Hash.ToLowerInvariant()
    }
}

$Gitleaks = (& (Join-Path $PSScriptRoot "install-gitleaks.ps1") `
        -RepositoryRoot $Repository | Select-Object -Last 1)
if (-not (Test-Path -LiteralPath $Gitleaks -PathType Leaf)) {
    throw "Gitleaks bootstrap did not return an executable"
}
$ToolVersion = (@(& $Gitleaks version) -join "").Trim()
if ($LASTEXITCODE -ne 0 -or $ToolVersion -ne "8.30.1") {
    throw "Gitleaks version does not match the reviewed release"
}
$StateBefore = Get-SecretScanState
if ($StateBefore.Dirty) {
    throw "secret scan requires one clean commit"
}

& (Join-Path $PSScriptRoot "validate-product-package.ps1") `
    -RepositoryRoot $Repository `
    -PackagePath $Package
if ($LASTEXITCODE -ne 0) {
    throw "closed product package validation failed"
}

if (Test-Path -LiteralPath $ScratchRoot) {
    Remove-TaskDirectory -Path $ScratchRoot -AllowedRoot (Join-Path $Repository "target")
}
New-Item -ItemType Directory -Path $ScratchRoot -Force | Out-Null
try {
    & $Gitleaks git `
        --redact `
        --no-banner `
        --no-color `
        --log-level error `
        --timeout 300 `
        --max-target-megabytes 128 `
        --config $Config `
        --gitleaks-ignore-path $Ignore `
        --report-format json `
        --report-path $SourceReport `
        $Repository
    if ($LASTEXITCODE -ne 0) {
        throw "committed source secret scan failed"
    }

    & $Gitleaks dir `
        --redact `
        --no-banner `
        --no-color `
        --log-level error `
        --timeout 300 `
        --max-archive-depth 1 `
        --max-target-megabytes 128 `
        --config $Config `
        --gitleaks-ignore-path $Ignore `
        --report-format json `
        --report-path $PackageReport `
        $Package
    if ($LASTEXITCODE -ne 0) {
        throw "closed package secret scan failed"
    }
}
finally {
    if (Test-Path -LiteralPath $ScratchRoot) {
        Remove-TaskDirectory -Path $ScratchRoot -AllowedRoot (Join-Path $Repository "target")
    }
}

$StateAfter = Get-SecretScanState
foreach ($Property in @(
        "Commit",
        "Dirty",
        "ToolSha256",
        "PackageSha256",
        "ConfigSha256",
        "IgnoreSha256"
    )) {
    if ($StateBefore.$Property -ne $StateAfter.$Property) {
        throw "secret scan inputs changed while the check was running"
    }
}

New-Item -ItemType Directory -Path $ReportRoot -Force | Out-Null
$Receipt = [ordered]@{
    schemaVersion = 1
    kind = "secret-scan"
    result = "pass"
    commit = $StateBefore.Commit
    dirty = $StateBefore.Dirty
    tool = "gitleaks"
    toolVersion = $ToolVersion
    toolSha256 = $StateBefore.ToolSha256
    packageSha256 = $StateBefore.PackageSha256
    configSha256 = $StateBefore.ConfigSha256
    ignoreSha256 = $StateBefore.IgnoreSha256
    sourceMode = "committed-git-history"
    packageMode = "validated-closed-zip-archive-depth-1"
}
$Destination = Join-Path $ReportRoot "secret-scan.json"
$Temporary = "$Destination.tmp"
$Receipt | ConvertTo-Json -Depth 4 |
    Set-Content -LiteralPath $Temporary -Encoding utf8NoBOM
Move-Item -LiteralPath $Temporary -Destination $Destination -Force
Write-Output "secret-scan-pass"
