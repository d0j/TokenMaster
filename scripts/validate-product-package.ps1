param(
    [Parameter(Mandatory)]
    [string]$PackagePath,

    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

. (Join-Path $PSScriptRoot "product-package-lib.ps1")
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

$Repository = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $RepositoryRoot).Path)
$Package = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $PackagePath).Path)
$TemporaryRoot = $null
Push-Location $Repository
try {
    $Dirty = & git status --porcelain
    if ($LASTEXITCODE -ne 0) {
        throw "git status failed"
    }
    if ($Dirty) {
        throw "product package validation requires one clean commit"
    }
    $Commit = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $Commit -notmatch '^[0-9a-f]{40}$') {
        throw "cannot resolve package validation commit"
    }

    $Input = [IO.File]::Open(
        $Package,
        [IO.FileMode]::Open,
        [IO.FileAccess]::Read,
        [IO.FileShare]::Read
    )
    try {
        $Archive = [IO.Compression.ZipArchive]::new(
            $Input,
            [IO.Compression.ZipArchiveMode]::Read,
            $false
        )
        try {
            if ($Archive.Entries.Count -ne 9) {
                throw "product ZIP content list is not closed"
            }
            $Names = [Collections.Generic.List[string]]::new()
            $Seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
            $RootName = $null
            foreach ($Entry in $Archive.Entries) {
                $Name = $Entry.FullName
                if ($Name.Contains('\') -or
                    $Name.StartsWith('/') -or
                    $Name.Contains(':') -or
                    $Name.Split('/') -contains '..' -or
                    $Name.EndsWith('/')) {
                    throw "product ZIP contains an unsafe entry"
                }
                $Segments = $Name.Split('/')
                if ($Segments.Count -ne 2 -or
                    $Segments[0] -notmatch '^TokenMaster-\d+\.\d+\.\d+-windows-x64$') {
                    throw "product ZIP entry is outside the canonical package root"
                }
                if ($null -eq $RootName) {
                    $RootName = $Segments[0]
                }
                elseif ($RootName -cne $Segments[0]) {
                    throw "product ZIP has more than one package root"
                }
                if (-not $Seen.Add($Name)) {
                    throw "product ZIP contains duplicate entries"
                }
                if (-not (Test-DeterministicZipTimestamp `
                    -Timestamp $Entry.LastWriteTime)) {
                    throw "product ZIP entry timestamp is not deterministic"
                }
                $Names.Add($Name)
            }
            $SortedNames = $Names.ToArray()
            [Array]::Sort($SortedNames, [StringComparer]::Ordinal)
            for ($Index = 0; $Index -lt $Names.Count; $Index++) {
                if ($Names[$Index] -cne $SortedNames[$Index]) {
                    throw "product ZIP entries are not canonically ordered"
                }
            }
        }
        finally {
            $Archive.Dispose()
        }
    }
    finally {
        $Input.Dispose()
    }

    $TemporaryRoot = [IO.Directory]::CreateTempSubdirectory(
        "tokenmaster-product-validate-"
    ).FullName
    [IO.Compression.ZipFile]::ExtractToDirectory($Package, $TemporaryRoot)
    $Stage = Join-Path $TemporaryRoot $RootName
    Assert-ProductPackageStage -StagePath $Stage

    $BuildInfo = Get-Content -LiteralPath (Join-Path $Stage "BUILDINFO.json") -Raw |
        ConvertFrom-Json
    if ($BuildInfo.commit -cne $Commit) {
        throw "product package commit does not match the clean validation commit"
    }
    $TargetDirectory = Get-CanonicalProductTargetDirectory -RepositoryRoot $Repository
    $CanonicalExecutable = Join-Path $TargetDirectory `
        "x86_64-pc-windows-msvc\release\TokenMaster.exe"
    if (-not (Test-Path -LiteralPath $CanonicalExecutable -PathType Leaf)) {
        throw "canonical MSVC build is unavailable"
    }
    $CanonicalHash = (Get-FileHash -LiteralPath $CanonicalExecutable -Algorithm SHA256).
        Hash.ToLowerInvariant()
    $PackagedHash = (Get-FileHash -LiteralPath (Join-Path $Stage "TokenMaster.exe") `
        -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($PackagedHash -cne $CanonicalHash) {
        throw "executable hash does not match canonical MSVC build"
    }
    & (Join-Path $PSScriptRoot "validate-msvc-product-binary.ps1") `
        -ExecutablePath $CanonicalExecutable
    if ($LASTEXITCODE -ne 0) {
        throw "canonical MSVC binary inspection failed"
    }
    Write-Host "product-package-validation-pass"
}
finally {
    Pop-Location
    if ($null -ne $TemporaryRoot -and [IO.Directory]::Exists($TemporaryRoot)) {
        [IO.Directory]::Delete($TemporaryRoot, $true)
    }
}
