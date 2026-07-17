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

function Get-ProductionRustText {
    param([Parameter(Mandatory = $true)][string]$Path)

    $text = [System.IO.File]::ReadAllText($Path)
    $testBoundary = $text.IndexOf('#[cfg(test)]', [System.StringComparison]::Ordinal)
    if ($testBoundary -ge 0) {
        return $text.Substring(0, $testBoundary)
    }
    return $text
}

$metadataJson = & cargo +1.97.0 metadata --locked --format-version 1 --manifest-path $manifest
if ($LASTEXITCODE -ne 0) {
    throw 'cargo metadata failed'
}
$metadata = $metadataJson | ConvertFrom-Json -Depth 100
$productPackages = @($metadata.packages | Where-Object { $_.name -eq 'tokenmaster-product' })
if ($productPackages.Count -ne 1) {
    throw 'tokenmaster-product was not resolved exactly once'
}
$productPackage = $productPackages[0]
$directProductionDependencies = @(
    $productPackage.dependencies |
        Where-Object { $null -eq $_.kind } |
        ForEach-Object { $_.name } |
        Sort-Object -Unique
)
$expectedDirectDependencies = @('tokenmaster-engine', 'tokenmaster-query', 'tokenmaster-runtime')
$unexpectedDirectDependencies = @(
    $directProductionDependencies | Where-Object { $_ -notin $expectedDirectDependencies }
)
$missingDirectDependencies = @(
    $expectedDirectDependencies | Where-Object { $_ -notin $directProductionDependencies }
)
if (
    $unexpectedDirectDependencies.Count -ne 0 -or
    $missingDirectDependencies.Count -ne 0 -or
    $directProductionDependencies.Count -ne $expectedDirectDependencies.Count
) {
    throw "product direct production dependencies drifted: $($directProductionDependencies -join ', ')"
}

$productRoot = Join-Path $root 'crates\product\src'
$productFiles = @(Get-ChildItem -LiteralPath $productRoot -File)
if ($productFiles.Count -ne 6 -or @($productFiles | Where-Object { $_.Extension -ne '.rs' }).Count -ne 0) {
    throw 'product production source must remain exactly six Rust files'
}
$productText = ($productFiles | ForEach-Object {
    Get-ProductionRustText -Path $_.FullName
}) -join "`n"
$runtimePath = Join-Path $productRoot 'runtime.rs'
$reducerPath = Join-Path $productRoot 'reducer.rs'
$snapshotPath = Join-Path $productRoot 'snapshot.rs'
$routePath = Join-Path $productRoot 'route.rs'
$runtimeText = Get-ProductionRustText -Path $runtimePath
$stateText = @(
    Get-ProductionRustText -Path $reducerPath
    Get-ProductionRustText -Path $snapshotPath
    Get-ProductionRustText -Path (Join-Path $productRoot 'section.rs')
    $runtimeText
    Get-ProductionRustText -Path $routePath
) -join "`n"

$forbiddenAuthorityPattern = @(
    'https?://',
    '\bstd::(fs|net|path|process)\b',
    '\b(Path|PathBuf|Command|TcpStream|TcpListener|UdpSocket)\b',
    '\b(rusqlite|slint|notify|reqwest|ureq|serde_json)\b',
    '\b(SELECT|INSERT|UPDATE|DELETE\s+FROM|PRAGMA)\b',
    'auth\.json|[\\/]\.codex[\\/]auth|\bAuthorization\b|\bBearer\s',
    'powershell(?:\.exe)?|cmd(?:\.exe)?|bash(?:\.exe)?|\bsh\s+-c\b'
) -join '|'
if ($productText -match $forbiddenAuthorityPattern) {
    throw 'product projection contains filesystem/network/process/SQL/UI/credential authority'
}
if ($runtimeText -match '\b(LiveRuntime|CodexQuotaRuntime|BenefitReminderRuntime|GitRuntime|RuntimeWriterLease|RefreshHintSink)\b') {
    throw 'product runtime projection retains or names a runtime owner instead of snapshots'
}
if ($runtimeText -match 'PathBuf|RepositoryCandidate|repository_id|association_id|account_id|window_id|lot_id|email|stderr|raw_output|source_contents') {
    throw 'product runtime health exposes path, identity, account, raw output, or source state'
}
if ($stateText -match '\b(Vec|VecDeque|HashMap|BTreeMap|LinkedList)<') {
    throw 'product current-state projection contains an unbounded or history-capable collection'
}
if ($productText -match '\bunsafe\b') {
    throw 'product production source contains unsafe code'
}

foreach ($pattern in @(
    'current: Arc<ProductSnapshot>',
    'ProductAttemptGeneration',
    'ProductRuntimeGeneration',
    'ProductRuntimeSection',
    'pub const ALL: \[Self; 11\]',
    'ProductRouteReasons\(u16\)',
    'invalidate_incompatible_sections',
    'next\.refresh_routes\(\)'
)) {
    if ($productText -notmatch $pattern) {
        throw "product projection is missing required constant-state behavior: $pattern"
    }
}

$statusStorePath = Join-Path $root 'crates\store\src\usage\query\status.rs'
$statusQueryPath = Join-Path $root 'crates\query\src\status.rs'
$statusStoreText = Get-ProductionRustText -Path $statusStorePath
$statusQueryText = Get-ProductionRustText -Path $statusQueryPath
foreach ($forbidden in @(
    'FROM usage_event',
    'JOIN usage_event',
    'usage_aggregate_time',
    'usage_aggregate_session',
    'quota_sample',
    'quota_transition',
    'benefit_change',
    'git_repository_day',
    'git_activity_association'
)) {
    if ($statusStoreText.IndexOf($forbidden, [System.StringComparison]::Ordinal) -ge 0) {
        throw "product status performs a forbidden archive scan: $forbidden"
    }
}
if ($statusQueryText -match 'PathBuf|std::path|rusqlite|\bSELECT\b|installation_salt|account_id|repository_id') {
    throw 'public product status exposes path, SQL, installation, account, or repository authority'
}

$contractFiles = @(
    Get-Item -LiteralPath $statusStorePath
    Get-Item -LiteralPath (Join-Path $root 'crates\store\tests\product_status_contract.rs')
    Get-Item -LiteralPath (Join-Path $root 'crates\query\tests\product_status_contract.rs')
    Get-Item -LiteralPath (Join-Path $root 'crates\query\tests\product_status_scale_contract.rs')
    Get-ChildItem -LiteralPath (Join-Path $root 'crates\product\tests') -File -Filter '*.rs'
)
$contractText = ($contractFiles | ForEach-Object {
    [System.IO.File]::ReadAllText($_.FullName)
}) -join "`n"
foreach ($needle in @(
    'capture_keeps_one_transaction_when_an_independent_revision_commits_mid_read',
    'aggregate_rebuilding_is_visible_without_hiding_archive_status',
    'large_archive_product_status_is_constant_plan_and_below_twenty_five_ms_p95',
    'reducer_accepts_only_newer_sections_and_keeps_faults_independent',
    'incompatible_async_results_are_rejected_and_new_status_invalidates_old_payloads',
    'runtime_observation_failures_are_ordered_and_fault_only_owned_routes',
    'ten_thousand_replacements_retain_one_fixed_product_snapshot',
    'repeated_status_open_capture_drop_returns_process_resources'
)) {
    if ($contractText.IndexOf($needle, [System.StringComparison]::Ordinal) -lt 0) {
        throw "product status acceptance contract is missing vector: $needle"
    }
}

$thirdPartyFiles = @(
    Get-ChildItem -LiteralPath (Join-Path $root 'third_party') -Recurse -File |
        ForEach-Object { $_.FullName.Substring($root.Length + 1).Replace('\', '/') }
)
$allowedThirdPartyFiles = @(
    'third_party/UPSTREAM.toml',
    'third_party/licenses/WhereMyTokens-MIT.txt',
    'third_party/licenses/ccusage-MIT.txt'
)
$unexpectedThirdParty = @($thirdPartyFiles | Where-Object { $_ -notin $allowedThirdPartyFiles })
if ($unexpectedThirdParty.Count -ne 0 -or $thirdPartyFiles.Count -ne $allowedThirdPartyFiles.Count) {
    throw 'third_party contains vendored or unexpected upstream source'
}

& cargo +1.97.0 build --release --locked --manifest-path $manifest -p tokenmaster-product
if ($LASTEXITCODE -ne 0) {
    throw 'release product library build failed'
}
$targetDirectory = [System.IO.Path]::GetFullPath([string]$metadata.target_directory)
$artifact = Get-ChildItem -LiteralPath $targetDirectory -Recurse -File |
    Where-Object {
        ($_.FullName -match '[\\/]release[\\/]deps[\\/]') -and
        ($_.Name -match '^(lib)?tokenmaster_product-.*\.rlib$')
    } |
    Sort-Object LastWriteTimeUtc -Descending |
    Select-Object -First 1
if ($null -eq $artifact) {
    throw 'release tokenmaster-product artifact was not found'
}
$artifactText = [System.Text.Encoding]::ASCII.GetString(
    [System.IO.File]::ReadAllBytes($artifact.FullName)
)
foreach ($needle in @(
    'PRIVATE_GIT_RUNTIME_REPOSITORY',
    'runtime@example.com',
    'status-scale-source',
    'SystemRoot',
    'where.exe',
    'SELECT count(*) FROM usage_event',
    'Authorization: Bearer',
    'auth.json'
)) {
    if ($artifactText.IndexOf($needle, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "release product artifact contains forbidden private/authority string: $needle"
    }
}

[ordered]@{
    result = 'pass'
    package = 'tokenmaster-product'
    direct_production_dependencies = $directProductionDependencies
    production_source_file_count = $productFiles.Count
    foreign_production_source_file_count = 0
    dynamic_state_collection_count = 0
    runtime_owner_count = 0
    direct_filesystem_network_process_sql_ui_authority = $false
    forbidden_status_scan_count = 0
    fixed_route_count = 11
    route_reason_capacity = 16
    vendored_upstream_source_count = 0
    release_artifact_count = 1
    forbidden_binary_string_count = 0
} | ConvertTo-Json -Compress
