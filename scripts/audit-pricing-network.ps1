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

$roots = @(
    $metadata.packages | Where-Object { $_.name -in @('tokenmaster-pricing', 'tokenmaster-query') }
)
if ($roots.Count -ne 2) {
    throw 'pricing/query packages were not resolved exactly once'
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
    foreach ($dependencyId in $nodesById[$id].dependencies) {
        $pending.Enqueue($dependencyId)
    }
}

$forbiddenCrates = @(
    'reqwest', 'ureq', 'hyper', 'hyper-util', 'h2', 'curl', 'isahc', 'surf',
    'awc', 'tokio', 'async-std', 'smol'
)
$dependencyNames = @(
    $visited | ForEach-Object { $packagesById[$_].name } | Sort-Object -Unique
)
$forbiddenDependencies = @($dependencyNames | Where-Object { $_ -in $forbiddenCrates })
if ($forbiddenDependencies.Count -ne 0) {
    throw "pricing dependency closure contains network/async crates: $($forbiddenDependencies -join ', ')"
}

$sourceFiles = @(
    Get-ChildItem -LiteralPath (Join-Path $root 'crates\pricing') -Recurse -File |
        Where-Object { $_.Extension -in @('.rs', '.toml') }
    Get-ChildItem -LiteralPath (Join-Path $root 'crates\query') -Recurse -File |
        Where-Object { $_.Extension -in @('.rs', '.toml') }
)
$sourceMatches = @(
    $sourceFiles | Select-String -Pattern 'models\.dev|litellm|https?://' -CaseSensitive:$false
)
if ($sourceMatches.Count -ne 0) {
    throw 'pricing/query production source contains a runtime network locator'
}

& cargo +1.97.0 build --release --locked --manifest-path $manifest `
    -p tokenmaster-pricing -p tokenmaster-query
if ($LASTEXITCODE -ne 0) {
    throw 'release pricing/query build failed'
}

$targetDirectory = [System.IO.Path]::GetFullPath([string]$metadata.target_directory)
$artifacts = @(
    Get-ChildItem -LiteralPath $targetDirectory -Recurse -File |
        Where-Object {
            ($_.FullName -match '[\\/]release[\\/]deps[\\/]') -and
            ($_.Name -match '^(lib)?tokenmaster_(pricing|query)-.*\.(rlib|rmeta)$')
        }
)
if ($artifacts.Count -lt 2) {
    throw 'release pricing/query library artifacts were not found'
}

$forbiddenBinaryStrings = @('models.dev', 'litellm', 'http://', 'https://')
foreach ($artifact in $artifacts) {
    $bytes = [System.IO.File]::ReadAllBytes($artifact.FullName)
    $text = [System.Text.Encoding]::ASCII.GetString($bytes)
    foreach ($needle in $forbiddenBinaryStrings) {
        if ($text.IndexOf($needle, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "release artifact contains forbidden pricing-network string: $needle"
        }
    }
}

[ordered]@{
    result = 'pass'
    packages = @('tokenmaster-pricing', 'tokenmaster-query')
    dependency_count = $dependencyNames.Count
    forbidden_dependency_count = 0
    production_source_file_count = $sourceFiles.Count
    release_artifact_count = $artifacts.Count
    forbidden_binary_string_count = 0
} | ConvertTo-Json -Compress
