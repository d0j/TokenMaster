param(
    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "release-tooling.ps1")
Add-Type -AssemblyName System.IO.Compression.FileSystem

$Version = "8.30.1"
$ArchiveName = "gitleaks_8.30.1_windows_x64.zip"
$ArchiveSha256 = "d29144deff3a68aa93ced33dddf84b7fdc26070add4aa0f4513094c8332afc4e"
$ExecutableSha256 = "17157e2ee8b76fc8b1d8bee607a250e34b8a8023c8bc81822d4b5ee4d78fcb7c"
$MaximumArchiveBytes = 9437184
$ArchiveUri = "https://github.com/gitleaks/gitleaks/releases/download/v$Version/$ArchiveName"
$ToolRoot = Join-Path $RepositoryRoot "target\tools\gitleaks\$Version"
$Executable = Join-Path $ToolRoot "gitleaks.exe"
$RepositoryPath = [IO.Path]::GetFullPath($RepositoryRoot)
$ToolPath = [IO.Path]::GetFullPath($ToolRoot)
if (-not $ToolPath.StartsWith(
        $RepositoryPath + [IO.Path]::DirectorySeparatorChar,
        [StringComparison]::OrdinalIgnoreCase
    )) {
    throw "Gitleaks tool root must remain inside the repository"
}

if (Test-Path -LiteralPath $Executable -PathType Leaf) {
    $ExistingHash = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).
        Hash.ToLowerInvariant()
    if ($ExistingHash -eq $ExecutableSha256) {
        Write-Output $Executable
        exit 0
    }
}

if (Test-Path -LiteralPath $ToolRoot) {
    Remove-TaskDirectory -Path $ToolPath -AllowedRoot (Join-Path $RepositoryRoot "target")
}
New-Item -ItemType Directory -Path $ToolRoot -Force | Out-Null
$ArchivePath = Join-Path $ToolRoot $ArchiveName
try {
    Invoke-WebRequest -Uri $ArchiveUri -OutFile $ArchivePath
    $ArchiveFile = Get-Item -LiteralPath $ArchivePath
    if ($ArchiveFile.Length -le 0 -or $ArchiveFile.Length -gt $MaximumArchiveBytes) {
        throw "Gitleaks archive size is outside the fixed bound"
    }
    $ActualArchiveHash = (Get-FileHash -LiteralPath $ArchivePath -Algorithm SHA256).
        Hash.ToLowerInvariant()
    if ($ActualArchiveHash -ne $ArchiveSha256) {
        throw "Gitleaks archive digest does not match the reviewed release"
    }

    $ExpectedEntries = @("LICENSE", "README.md", "gitleaks.exe")
    $Archive = [IO.Compression.ZipFile]::OpenRead($ArchivePath)
    try {
        $Entries = @($Archive.Entries | ForEach-Object FullName)
        if ($Entries.Count -ne $ExpectedEntries.Count -or
            (Compare-Object -ReferenceObject $ExpectedEntries -DifferenceObject $Entries)) {
            throw "Gitleaks archive contents are not the reviewed closed set"
        }
    }
    finally {
        $Archive.Dispose()
    }
    [IO.Compression.ZipFile]::ExtractToDirectory($ArchivePath, $ToolRoot)
    $ActualExecutableHash = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).
        Hash.ToLowerInvariant()
    if ($ActualExecutableHash -ne $ExecutableSha256) {
        throw "Gitleaks executable digest does not match the reviewed release"
    }
}
finally {
    Remove-Item -LiteralPath $ArchivePath -Force -ErrorAction SilentlyContinue
}

Write-Output $Executable
