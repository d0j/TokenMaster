param(
    [string]$RepositoryRoot = (Get-Location).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if (-not [IO.Path]::IsPathRooted($RepositoryRoot) -or -not (Test-Path -LiteralPath $RepositoryRoot -PathType Container)) {
    throw "TM-CLEAN-INVALID-ROOT"
}

$Root = [IO.Path]::GetFullPath($RepositoryRoot)
$RootManifest = Join-Path $Root "Cargo.toml"
if (-not (Test-Path -LiteralPath $RootManifest -PathType Leaf) -or
    -not ((Get-Content -LiteralPath $RootManifest -Raw) -match '(?m)^\[workspace\]')) {
    throw "TM-CLEAN-INVALID-ROOT"
}

foreach ($ForbiddenDirectory in @("apps", "portable", "tokenmaster")) {
    if (Test-Path -LiteralPath (Join-Path $Root $ForbiddenDirectory) -PathType Container) {
        throw "TM-CLEAN-FORBIDDEN-ROOT"
    }
}

$IgnoredRoots = @(".git", ".worktrees", "target", "reports", "dist")
$Files = Get-ChildItem -LiteralPath $Root -Recurse -Force -File | Where-Object {
    $Relative = [IO.Path]::GetRelativePath($Root, $_.FullName).Replace('\', '/')
    -not ($IgnoredRoots | Where-Object { $Relative -eq $_ -or $Relative.StartsWith("$_/", [StringComparison]::OrdinalIgnoreCase) })
}

foreach ($File in $Files) {
    $Relative = [IO.Path]::GetRelativePath($Root, $File.FullName).Replace('\', '/')
    if ($Relative -match '(^|/)(go\.mod|package\.json)$') {
        throw "TM-CLEAN-FOREIGN-MANIFEST"
    }
    if ($Relative -eq "Cargo.toml") {
        continue
    }
    if ($Relative -like "*/Cargo.toml") {
        $Manifest = Get-Content -LiteralPath $File.FullName -Raw
        if ($Manifest -match '(?m)^\[workspace\]' -or $Relative -notmatch '^crates/[^/]+/Cargo\.toml$') {
            throw "TM-CLEAN-SECOND-WORKSPACE"
        }
    }
}

$TextExtensions = @(".md", ".toml", ".ps1", ".psm1", ".rs", ".slint", ".yml", ".yaml", ".json", ".txt", ".po", ".lock")
$LegacyIdentifier = "Codex" + "Scope"
foreach ($File in $Files) {
    if ($File.Name -ne "AGENTS.md" -and $File.Extension -notin $TextExtensions) {
        continue
    }
    if ((Get-Content -LiteralPath $File.FullName -Raw) -match [regex]::Escape($LegacyIdentifier)) {
        throw "TM-CLEAN-LEGACY-IDENTIFIER"
    }
}

Write-Output "TM-CLEAN-PASS"
