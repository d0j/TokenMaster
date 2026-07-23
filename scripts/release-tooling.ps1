Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Remove-TaskDirectory {
    param(
        [Parameter(Mandatory)]
        [string]$Path,
        [Parameter(Mandatory)]
        [string]$AllowedRoot
    )

    $ResolvedPath = [IO.Path]::GetFullPath($Path)
    $ResolvedRoot = [IO.Path]::GetFullPath($AllowedRoot).TrimEnd(
        [IO.Path]::DirectorySeparatorChar,
        [IO.Path]::AltDirectorySeparatorChar
    )
    if (-not $ResolvedPath.StartsWith(
            $ResolvedRoot + [IO.Path]::DirectorySeparatorChar,
            [StringComparison]::OrdinalIgnoreCase
        )) {
        throw "task directory must be a child of its declared root"
    }
    if (-not (Test-Path -LiteralPath $ResolvedPath)) {
        return
    }
    foreach ($File in Get-ChildItem -LiteralPath $ResolvedPath -Recurse -Force -File) {
        if ($File.IsReadOnly) {
            $File.IsReadOnly = $false
        }
    }
    [IO.Directory]::Delete($ResolvedPath, $true)
}

function Get-DependencyPolicyState {
    param(
        [Parameter(Mandatory)]
        [string]$RepositoryRoot,
        [Parameter(Mandatory)]
        [string]$CargoDenyPath,
        [Parameter(Mandatory)]
        [string]$PolicyPath,
        [Parameter(Mandatory)]
        [string]$LockPath
    )

    foreach ($RequiredFile in @($CargoDenyPath, $PolicyPath, $LockPath)) {
        if (-not (Test-Path -LiteralPath $RequiredFile -PathType Leaf)) {
            throw "dependency policy state input is missing"
        }
    }
    $Git = (Get-Command git.exe -CommandType Application -ErrorAction Stop |
            Select-Object -First 1).Source
    $Commit = (@(& $Git -C $RepositoryRoot rev-parse HEAD) -join "").Trim()
    if ($LASTEXITCODE -ne 0 -or $Commit -notmatch '^[0-9a-f]{40}$') {
        throw "dependency policy could not bind the current commit"
    }
    $Status = @(& $Git -C $RepositoryRoot status --porcelain)
    if ($LASTEXITCODE -ne 0) {
        throw "dependency policy could not bind the worktree state"
    }
    [pscustomobject]@{
        Commit = $Commit
        Dirty = $Status.Count -ne 0
        ToolSha256 = (Get-FileHash -LiteralPath $CargoDenyPath -Algorithm SHA256).Hash.ToLowerInvariant()
        PolicySha256 = (Get-FileHash -LiteralPath $PolicyPath -Algorithm SHA256).Hash.ToLowerInvariant()
        LockSha256 = (Get-FileHash -LiteralPath $LockPath -Algorithm SHA256).Hash.ToLowerInvariant()
    }
}

function Assert-DependencyPolicyStateUnchanged {
    param(
        [Parameter(Mandatory)]
        [psobject]$Before,
        [Parameter(Mandatory)]
        [psobject]$After
    )

    foreach ($Property in @(
            "Commit",
            "Dirty",
            "ToolSha256",
            "PolicySha256",
            "LockSha256"
        )) {
        if ($Before.$Property -ne $After.$Property) {
            throw "dependency policy inputs changed while the check was running"
        }
    }
}
