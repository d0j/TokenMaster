param(
    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

Push-Location $RepositoryRoot
try {
    $Dirty = & git status --porcelain
    if ($LASTEXITCODE -ne 0) { throw "git status failed" }
    if ($Dirty) { throw "M0 packaging requires one clean commit" }

    & (Join-Path $PSScriptRoot "verify-m0.ps1") -RepositoryRoot $RepositoryRoot
    $Commit = (& git rev-parse HEAD).Trim()
    $ShortCommit = (& git rev-parse --short=12 HEAD).Trim()
    $Executable = Join-Path $RepositoryRoot "target\x86_64-pc-windows-gnu\release\tokenmaster-m0.exe"
    $ExecutableSha256 = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).Hash.ToLowerInvariant()
    $ReportRoot = Join-Path $RepositoryRoot "reports"
    $Interactive = Join-Path $ReportRoot "interactive-m0.json"
    $Soak = Join-Path $ReportRoot "soak-24h.json"
    foreach ($Required in @($Interactive, $Soak)) {
        if (-not (Test-Path -LiteralPath $Required)) { throw "missing required external receipt: $Required" }
        $Receipt = Get-Content -LiteralPath $Required -Raw | ConvertFrom-Json
        if ($Receipt.result -ne "pass") { throw "external receipt is not PASS: $Required" }
        if ($Receipt.commit -ne $Commit) { throw "external receipt commit mismatch: $Required" }
        if ($Receipt.dirty -ne $false) { throw "external receipt was captured from a dirty tree: $Required" }
        if ($Receipt.executableSha256 -ne $ExecutableSha256) { throw "external receipt executable mismatch: $Required" }
    }

    $Dist = [IO.Path]::GetFullPath((Join-Path $RepositoryRoot "dist"))
    $Stage = [IO.Path]::GetFullPath((Join-Path $Dist "tokenmaster-m0-$ShortCommit-windows-x64"))
    $DistPrefix = $Dist.TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
    if (-not $Stage.StartsWith($DistPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "stage path escapes the package directory"
    }
    if (Test-Path -LiteralPath $Stage) { Remove-Item -LiteralPath $Stage -Recurse -Force }
    New-Item -ItemType Directory -Path (Join-Path $Stage "reports") -Force | Out-Null
    Copy-Item (Join-Path $RepositoryRoot "target\x86_64-pc-windows-gnu\release\tokenmaster-m0.exe") $Stage
    Copy-Item (Join-Path $RepositoryRoot "README.md") $Stage
    Copy-Item (Join-Path $RepositoryRoot "M0_ACCEPTANCE.md") $Stage
    Copy-Item (Join-Path $ReportRoot "verification-summary.json") (Join-Path $Stage "reports")
    Copy-Item $Interactive (Join-Path $Stage "reports")
    Copy-Item $Soak (Join-Path $Stage "reports")

    [ordered]@{
        schemaVersion = 1
        status = "M0 architecture proof"
        commit = $Commit
        rust = (& rustc +1.97.0 --version)
        builtUtc = [DateTimeOffset]::UtcNow.ToString("O")
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Stage "BUILDINFO.json") -Encoding utf8NoBOM

    Get-ChildItem $Stage -Recurse -File | Sort-Object FullName | ForEach-Object {
        $Relative = [IO.Path]::GetRelativePath($Stage, $_.FullName).Replace('\','/')
        "{0}  {1}" -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant(), $Relative
    } | Set-Content -LiteralPath (Join-Path $Stage "SHA256SUMS.txt") -Encoding ascii
    $Zip = Join-Path $Dist "tokenmaster-m0-$ShortCommit-windows-x64.zip"
    Compress-Archive -Path $Stage -DestinationPath $Zip -CompressionLevel Optimal
    Write-Host "Created non-release M0 architecture proof: $Zip"
}
finally {
    Pop-Location
}
