[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$RepositoryRoot
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$manifest = Join-Path $root 'Cargo.toml'
if (-not (Test-Path -LiteralPath $manifest -PathType Leaf)) {
    throw 'repository root does not contain Cargo.toml'
}

$metadataJson = & cargo +1.97.0 metadata --locked --format-version 1 --manifest-path $manifest
if ($LASTEXITCODE -ne 0) {
    throw 'cargo metadata failed'
}
$metadata = $metadataJson | ConvertFrom-Json -Depth 100
$packagesById = @{}
foreach ($package in $metadata.packages) {
    $packagesById[$package.id] = $package
}
$nodesById = @{}
foreach ($node in $metadata.resolve.nodes) {
    $nodesById[$node.id] = $node
}

$rootNames = @('tokenmaster-quota', 'tokenmaster-store', 'tokenmaster-query')
$roots = @($metadata.packages | Where-Object { $_.name -in $rootNames })
if ($roots.Count -ne $rootNames.Count) {
    throw 'quota/store/query packages were not resolved exactly once'
}

$visited = [System.Collections.Generic.HashSet[string]]::new()
$pending = [System.Collections.Generic.Queue[string]]::new()
foreach ($package in $roots) {
    $pending.Enqueue($package.id)
}
while ($pending.Count -gt 0) {
    $id = $pending.Dequeue()
    if (-not $visited.Add($id)) {
        continue
    }
    foreach ($dependency in $nodesById[$id].deps) {
        $productionKinds = @($dependency.dep_kinds | Where-Object { $_.kind -ne 'dev' })
        if ($productionKinds.Count -ne 0) {
            $pending.Enqueue($dependency.pkg)
        }
    }
}

$forbiddenCrates = @(
    'reqwest', 'ureq', 'hyper', 'hyper-util', 'h2', 'curl', 'isahc', 'surf',
    'awc', 'tokio', 'async-std', 'smol', 'webbrowser', 'headless_chrome',
    'chromiumoxide', 'thirtyfour', 'fantoccini', 'cookie_store'
)
$dependencyNames = @(
    $visited | ForEach-Object { $packagesById[$_].name } | Sort-Object -Unique
)
$forbiddenDependencies = @($dependencyNames | Where-Object { $_ -in $forbiddenCrates })
if ($forbiddenDependencies.Count -ne 0) {
    throw "quota dependency closure contains network/browser/async crates: $($forbiddenDependencies -join ', ')"
}

$sourceFiles = @()
foreach ($crate in @('quota', 'store', 'query')) {
    $crateRoot = Join-Path $root "crates\$crate"
    $sourceFiles += Get-ChildItem -LiteralPath (Join-Path $crateRoot 'src') -Recurse -File |
        Where-Object { $_.Extension -eq '.rs' }
    $sourceFiles += Get-Item -LiteralPath (Join-Path $crateRoot 'Cargo.toml')
}
$forbiddenSourcePattern = @(
    'https?://',
    '\b(reqwest|ureq|isahc|fantoccini|chromiumoxide|thirtyfour)\b',
    '\b(hyper|curl|surf|awc)::',
    'headless[_-]?chrome|webbrowser|cookie[_-]?store|set-cookie',
    'private[_ -]?endpoint',
    'std::process::Command',
    'powershell(?:\.exe)?|cmd(?:\.exe)?',
    '\bTcpStream\b|\bUdpSocket\b'
) -join '|'
$sourceMatches = @(
    $sourceFiles | Select-String -Pattern $forbiddenSourcePattern -CaseSensitive:$false
)
if ($sourceMatches.Count -ne 0) {
    throw 'quota/store/query production source contains forbidden network/browser/shell authority'
}

& cargo +1.97.0 build --release --locked --manifest-path $manifest `
    -p tokenmaster-quota -p tokenmaster-store -p tokenmaster-query
if ($LASTEXITCODE -ne 0) {
    throw 'release quota/store/query build failed'
}

$targetDirectory = [System.IO.Path]::GetFullPath([string]$metadata.target_directory)
$artifacts = @()
foreach ($crate in @('quota', 'store', 'query')) {
    $artifact = Get-ChildItem -LiteralPath $targetDirectory -Recurse -File |
        Where-Object {
            ($_.FullName -match '[\\/]release[\\/]deps[\\/]') -and
            ($_.Name -match "^(lib)?tokenmaster_$crate-.*\.rlib$")
        } |
        Sort-Object LastWriteTimeUtc -Descending |
        Select-Object -First 1
    if ($null -eq $artifact) {
        throw "release tokenmaster-$crate library artifact was not found"
    }
    $artifacts += $artifact
}

$forbiddenBinaryStrings = @(
    'http://', 'https://', 'Set-Cookie', 'cookie_store', 'private endpoint',
    'powershell.exe', 'cmd.exe', 'TcpStream', 'UdpSocket'
)
foreach ($artifact in $artifacts) {
    $bytes = [System.IO.File]::ReadAllBytes($artifact.FullName)
    $text = [System.Text.Encoding]::ASCII.GetString($bytes)
    foreach ($needle in $forbiddenBinaryStrings) {
        if ($text.IndexOf($needle, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "release artifact contains forbidden quota-authority string: $needle"
        }
    }
}

[ordered]@{
    result = 'pass'
    packages = $rootNames
    dependency_count = $dependencyNames.Count
    forbidden_dependency_count = 0
    production_source_file_count = $sourceFiles.Count
    release_artifact_count = $artifacts.Count
    forbidden_binary_string_count = 0
} | ConvertTo-Json -Compress
