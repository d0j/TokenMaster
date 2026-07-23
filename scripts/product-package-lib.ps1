Set-StrictMode -Version Latest

function Get-CanonicalProductTargetDirectory {
    param(
        [Parameter(Mandatory)]
        [string]$RepositoryRoot
    )

    return [IO.Path]::GetFullPath((Join-Path $RepositoryRoot "target"))
}

function Write-Utf8NoBomFile {
    param(
        [Parameter(Mandatory)]
        [string]$Path,

        [Parameter(Mandatory)]
        [AllowEmptyString()]
        [string]$Content
    )

    $Normalized = $Content.Replace("`r`n", "`n").Replace("`r", "`n")
    [IO.File]::WriteAllText(
        [IO.Path]::GetFullPath($Path),
        $Normalized,
        [Text.UTF8Encoding]::new($false)
    )
}

function Test-DeterministicZipTimestamp {
    param(
        [Parameter(Mandatory)]
        [DateTimeOffset]$Timestamp
    )

    return $Timestamp.Year -eq 1980 -and
        $Timestamp.Month -eq 1 -and
        $Timestamp.Day -eq 1 -and
        $Timestamp.Hour -eq 0 -and
        $Timestamp.Minute -eq 0 -and
        $Timestamp.Second -eq 0
}

function Get-ProductStageRelativeFiles {
    param(
        [Parameter(Mandatory)]
        [string]$StagePath,

        [string[]]$ExcludedRelativePaths = @()
    )

    $ResolvedStage = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $StagePath).Path)
    $Excluded = [Collections.Generic.HashSet[string]]::new(
        [StringComparer]::Ordinal
    )
    foreach ($RelativePath in $ExcludedRelativePaths) {
        [void]$Excluded.Add($RelativePath.Replace('\', '/'))
    }

    $RelativeFiles = [Collections.Generic.List[string]]::new()
    foreach ($File in [IO.Directory]::EnumerateFiles(
        $ResolvedStage,
        "*",
        [IO.SearchOption]::AllDirectories
    )) {
        $Relative = [IO.Path]::GetRelativePath($ResolvedStage, $File).Replace('\', '/')
        if (-not $Excluded.Contains($Relative)) {
            $RelativeFiles.Add($Relative)
        }
    }
    $RelativeFiles.Sort([StringComparer]::Ordinal)
    return $RelativeFiles.ToArray()
}

function Write-ProductChecksums {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string]$StagePath
    )

    $ResolvedStage = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $StagePath).Path)
    $Lines = foreach ($Relative in Get-ProductStageRelativeFiles `
        -StagePath $ResolvedStage `
        -ExcludedRelativePaths @("SHA256SUMS.txt")) {
        $FilePath = [IO.Path]::GetFullPath((Join-Path $ResolvedStage $Relative))
        $Hash = (Get-FileHash -LiteralPath $FilePath -Algorithm SHA256).Hash.ToLowerInvariant()
        "$Hash  $Relative"
    }
    [IO.File]::WriteAllText(
        (Join-Path $ResolvedStage "SHA256SUMS.txt"),
        (($Lines -join "`n") + "`n"),
        [Text.Encoding]::ASCII
    )
}

function Assert-ProductPackageStage {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string]$StagePath
    )

    $ResolvedStage = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $StagePath).Path)
    if ([IO.Path]::GetFileName($ResolvedStage) -notmatch '^TokenMaster-\d+\.\d+\.\d+-windows-x64$') {
        throw "unexpected product package root name"
    }

    foreach ($Item in Get-ChildItem -LiteralPath $ResolvedStage -Recurse -Force) {
        if (($Item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "product package must not contain links or reparse points"
        }
    }

    $ExpectedFiles = @(
        "BUILDINFO.json",
        "LICENSE",
        "README.md",
        "README_RU.md",
        "SBOM.cdx.json",
        "SHA256SUMS.txt",
        "THIRD_PARTY_NOTICES.txt",
        "TokenMaster.exe",
        "tokenmaster.portable"
    )
    [Array]::Sort($ExpectedFiles, [StringComparer]::Ordinal)
    $ActualFiles = @(Get-ProductStageRelativeFiles -StagePath $ResolvedStage)
    if ($ActualFiles.Count -ne $ExpectedFiles.Count) {
        throw "product package content list is not closed"
    }
    for ($Index = 0; $Index -lt $ExpectedFiles.Count; $Index++) {
        if ($ActualFiles[$Index] -cne $ExpectedFiles[$Index]) {
            throw "product package content list is not closed"
        }
    }

    $PortableMarker = Join-Path $ResolvedStage "tokenmaster.portable"
    if ((Get-Item -LiteralPath $PortableMarker).Length -ne 0) {
        throw "portable marker must be empty"
    }
    $Executable = Join-Path $ResolvedStage "TokenMaster.exe"
    if ((Get-Item -LiteralPath $Executable).Length -eq 0) {
        throw "product executable must not be empty"
    }

    $BuildInfo = Get-Content -LiteralPath (Join-Path $ResolvedStage "BUILDINFO.json") -Raw |
        ConvertFrom-Json
    if ($BuildInfo.schemaVersion -ne 1 -or
        $BuildInfo.status -cne "unsigned package candidate" -or
        $BuildInfo.version -notmatch '^\d+\.\d+\.\d+$' -or
        $BuildInfo.commit -notmatch '^[0-9a-f]{40}$' -or
        $BuildInfo.target -cne "x86_64-pc-windows-msvc" -or
        $BuildInfo.executableSha256 -notmatch '^[0-9a-f]{64}$') {
        throw "invalid product build identity"
    }
    $ExecutableHash = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($BuildInfo.executableSha256 -cne $ExecutableHash) {
        throw "product executable hash does not match build identity"
    }

    $Sbom = Get-Content -LiteralPath (Join-Path $ResolvedStage "SBOM.cdx.json") -Raw |
        ConvertFrom-Json
    if ($Sbom.bomFormat -cne "CycloneDX" -or
        $Sbom.specVersion -cne "1.6" -or
        $Sbom.version -ne 1 -or
        @($Sbom.components).Count -eq 0) {
        throw "invalid or empty product SBOM"
    }
    foreach ($Component in @($Sbom.components)) {
        if ($Component.type -cne "library" -or
            [string]::IsNullOrWhiteSpace($Component.name) -or
            [string]::IsNullOrWhiteSpace($Component.version) -or
            $Component.purl -notmatch '^pkg:cargo/' -or
            @($Component.licenses).Count -eq 0) {
            throw "invalid product SBOM component"
        }
    }

    $Notices = Get-Content -LiteralPath (Join-Path $ResolvedStage "THIRD_PARTY_NOTICES.txt") -Raw
    if ([string]::IsNullOrWhiteSpace($Notices)) {
        throw "third-party notices must not be empty"
    }

    $TextFiles = @(
        "BUILDINFO.json",
        "README.md",
        "README_RU.md",
        "SBOM.cdx.json",
        "THIRD_PARTY_NOTICES.txt"
    )
    foreach ($TextFile in $TextFiles) {
        $Text = Get-Content -LiteralPath (Join-Path $ResolvedStage $TextFile) -Raw
        if ($Text -match '(?im)(?:^|[\s''"(])(?:[a-z]:[\\/]|\\\\\?\\)') {
            throw "product package text contains an absolute Windows path"
        }
    }

    $ManifestPath = Join-Path $ResolvedStage "SHA256SUMS.txt"
    $ManifestLines = @(Get-Content -LiteralPath $ManifestPath)
    $ManifestFiles = @(Get-ProductStageRelativeFiles `
        -StagePath $ResolvedStage `
        -ExcludedRelativePaths @("SHA256SUMS.txt"))
    if ($ManifestLines.Count -ne $ManifestFiles.Count) {
        throw "product checksum manifest is incomplete"
    }
    for ($Index = 0; $Index -lt $ManifestFiles.Count; $Index++) {
        if ($ManifestLines[$Index] -notmatch '^([0-9a-f]{64})  (.+)$' -or
            $Matches[2] -cne $ManifestFiles[$Index]) {
            throw "product checksum manifest is not canonical"
        }
        $ExpectedHash = (Get-FileHash `
            -LiteralPath (Join-Path $ResolvedStage $ManifestFiles[$Index]) `
            -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($Matches[1] -cne $ExpectedHash) {
            throw "product checksum manifest hash mismatch"
        }
    }
}

function New-DeterministicZip {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory)]
        [string]$StagePath,

        [Parameter(Mandatory)]
        [string]$DestinationPath
    )

    Add-Type -AssemblyName System.IO.Compression

    $ResolvedStage = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $StagePath).Path)
    $StageName = [IO.Path]::GetFileName($ResolvedStage)
    $Destination = [IO.Path]::GetFullPath($DestinationPath)
    $DestinationDirectory = [IO.Path]::GetDirectoryName($Destination)
    [IO.Directory]::CreateDirectory($DestinationDirectory) | Out-Null

    $Output = [IO.File]::Open(
        $Destination,
        [IO.FileMode]::Create,
        [IO.FileAccess]::Write,
        [IO.FileShare]::None
    )
    try {
        $Archive = [IO.Compression.ZipArchive]::new(
            $Output,
            [IO.Compression.ZipArchiveMode]::Create,
            $false
        )
        try {
            foreach ($Relative in Get-ProductStageRelativeFiles -StagePath $ResolvedStage) {
                $File = [IO.Path]::GetFullPath((Join-Path $ResolvedStage $Relative))
                $Entry = $Archive.CreateEntry(
                    "$StageName/$Relative",
                    [IO.Compression.CompressionLevel]::Optimal
                )
                $Entry.LastWriteTime = [DateTimeOffset]::new(
                    1980,
                    1,
                    1,
                    0,
                    0,
                    0,
                    [TimeSpan]::Zero
                )
                $Input = [IO.File]::Open(
                    $File,
                    [IO.FileMode]::Open,
                    [IO.FileAccess]::Read,
                    [IO.FileShare]::Read
                )
                try {
                    $EntryStream = $Entry.Open()
                    try {
                        $Input.CopyTo($EntryStream)
                    }
                    finally {
                        $EntryStream.Dispose()
                    }
                }
                finally {
                    $Input.Dispose()
                }
            }
        }
        finally {
            $Archive.Dispose()
        }
    }
    finally {
        $Output.Dispose()
    }
}
