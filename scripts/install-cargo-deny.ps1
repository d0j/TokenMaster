param(
    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
. (Join-Path $PSScriptRoot "release-tooling.ps1")

$Version = "0.20.2"
$ArchiveName = "cargo-deny-0.20.2-x86_64-pc-windows-msvc.tar.gz"
$ArchiveSha256 = "975a22143262fd27476d19ee00c7af67978426e40e1dee94eed6bbade1cf87dc"
$ExecutableSha256 = "f7292fab58c706638c999e64c4ba82e5128ae628130ba55e3266a768ee431fbf"
$MaximumArchiveBytes = 8388608
$ArchiveUri = "https://github.com/EmbarkStudios/cargo-deny/releases/download/$Version/$ArchiveName"
$ToolRoot = Join-Path $RepositoryRoot "target\tools\cargo-deny\$Version"
$ExtractedName = "cargo-deny-$Version-x86_64-pc-windows-msvc"
$Executable = Join-Path $ToolRoot "$ExtractedName\cargo-deny.exe"
$RepositoryPath = [IO.Path]::GetFullPath($RepositoryRoot)
$ToolPath = [IO.Path]::GetFullPath($ToolRoot)
if (-not $ToolPath.StartsWith(
        $RepositoryPath + [IO.Path]::DirectorySeparatorChar,
        [StringComparison]::OrdinalIgnoreCase
    )) {
    throw "cargo-deny tool root must remain inside the repository"
}

if (Test-Path -LiteralPath $Executable -PathType Leaf) {
    $ExistingHash = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash.ToLowerInvariant()
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
    $Archive = Get-Item -LiteralPath $ArchivePath
    if ($Archive.Length -le 0 -or $Archive.Length -gt $MaximumArchiveBytes) {
        throw "cargo-deny archive size is outside the fixed bound"
    }
    $ActualArchiveHash = (Get-FileHash -LiteralPath $ArchivePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($ActualArchiveHash -ne $ArchiveSha256) {
        throw "cargo-deny archive digest does not match the reviewed release"
    }

    $Tar = "C:\Windows\System32\tar.exe"
    if (-not (Test-Path -LiteralPath $Tar -PathType Leaf)) {
        throw "Windows system tar is required"
    }
    $ExpectedEntries = @(
        "$ExtractedName/",
        "$ExtractedName/cargo-deny.exe",
        "$ExtractedName/LICENSE-APACHE",
        "$ExtractedName/LICENSE-MIT",
        "$ExtractedName/README.md"
    )
    $Entries = @(& $Tar -tzf $ArchivePath)
    if ($LASTEXITCODE -ne 0 -or
        $Entries.Count -ne $ExpectedEntries.Count -or
        (Compare-Object -ReferenceObject $ExpectedEntries -DifferenceObject $Entries)) {
        throw "cargo-deny archive contents are not the reviewed closed set"
    }
    & $Tar -xzf $ArchivePath -C $ToolRoot
    if ($LASTEXITCODE -ne 0) {
        throw "cargo-deny archive extraction failed"
    }
    $ActualExecutableHash = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($ActualExecutableHash -ne $ExecutableSha256) {
        throw "cargo-deny executable digest does not match the reviewed release"
    }
}
finally {
    Remove-Item -LiteralPath $ArchivePath -Force -ErrorAction SilentlyContinue
}

Write-Output $Executable
