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
$rootPackages = @($metadata.packages | Where-Object { $_.name -eq 'tokenmaster-runtime' })
if ($rootPackages.Count -ne 1) {
    throw 'tokenmaster-runtime was not resolved exactly once'
}

$packagesById = @{}
foreach ($package in $metadata.packages) {
    $packagesById[$package.id] = $package
}
$nodesById = @{}
foreach ($node in $metadata.resolve.nodes) {
    $nodesById[$node.id] = $node
}
$visited = [System.Collections.Generic.HashSet[string]]::new()
$pending = [System.Collections.Generic.Queue[string]]::new()
$pending.Enqueue($rootPackages[0].id)
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
    throw "quota runtime dependency closure contains forbidden crates: $($forbiddenDependencies -join ', ')"
}

$quotaSourceRoot = Join-Path $root 'crates\runtime\src\quota'
$librarySourceFiles = @(Get-ChildItem -LiteralPath $quotaSourceRoot -Recurse -File)
$foreignSourceFiles = @(
    $librarySourceFiles | Where-Object { $_.Extension -ne '.rs' }
)
if ($foreignSourceFiles.Count -ne 0) {
    throw 'quota runtime production source contains a foreign-language file'
}
$forbiddenSourcePattern = @(
    'https?://',
    '\bstd::net\b|\bTcp(Stream|Listener)\b|\bUdpSocket\b',
    '\b(reqwest|ureq|isahc|fantoccini|chromiumoxide|thirtyfour)\b',
    '\b(hyper|curl|surf|awc)::',
    'headless[_-]?chrome|webbrowser|cookie[_-]?store|set-cookie',
    'auth\.json|[\\/]\.codex[\\/]auth',
    '\bAuthorization\b|\bBearer\s',
    'private[_ -]?endpoint',
    'powershell(?:\.exe)?|cmd(?:\.exe)?|bash(?:\.exe)?|\bsh\s+-c\b',
    '\bCommand::new\b',
    '\bTcpStream\b|\bUdpSocket\b',
    '\brusqlite\b|\btransaction_with_behavior\b'
) -join '|'
$sourceMatches = @(
    $librarySourceFiles | Select-String -Pattern $forbiddenSourcePattern -CaseSensitive:$false
)
if ($sourceMatches.Count -ne 0) {
    throw 'quota runtime source contains forbidden network/browser/credential/shell/direct-SQL authority'
}

$discoveryPath = Join-Path $quotaSourceRoot 'discovery.rs'
$configPath = Join-Path $quotaSourceRoot 'config.rs'
$executionPath = Join-Path $quotaSourceRoot 'execution.rs'
$runtimePath = Join-Path $quotaSourceRoot 'runtime.rs'
$healthPath = Join-Path $quotaSourceRoot 'health.rs'
$discoveryText = [System.IO.File]::ReadAllText($discoveryPath)
$configText = [System.IO.File]::ReadAllText($configPath)
$executionText = [System.IO.File]::ReadAllText($executionPath)
$runtimeText = [System.IO.File]::ReadAllText($runtimePath)
$healthText = [System.IO.File]::ReadAllText($healthPath)

$requiredDiscoveryPatterns = @(
    'MAX_CODEX_EXECUTABLE_SEARCH_DIRS',
    'MAX_CODEX_EXECUTABLE_SEARCH_PATH_BYTES',
    'env::split_paths\(&self\.raw\)',
    'directory\.is_absolute\(\)',
    'CodexAppServerCommand::new\(candidate\)',
    '"codex\.exe"',
    '"codex"'
)
foreach ($pattern in $requiredDiscoveryPatterns) {
    if ($discoveryText -notmatch $pattern) {
        throw "quota discovery is missing required boundary pattern: $pattern"
    }
}
if ($discoveryText -match '\.cmd|\.ps1|PATHEXT|(?<!AppServer)Command::new') {
    throw 'quota discovery contains script, PATHEXT, or direct process execution authority'
}
if ($configText -notmatch 'CodexExecutableSelection::Explicit\(command\) => Ok\(command\.clone\(\)\)') {
    throw 'explicit executable selection is not authoritative'
}
if ($configText -notmatch '\.field\("archive_path", &"\[redacted\]"\)') {
    throw 'quota runtime config Debug does not redact the archive path'
}

$sourcePollIndex = $executionText.IndexOf('self.source.poll(observed_at_ms)', [System.StringComparison]::Ordinal)
$publisherIndex = $executionText.IndexOf('self.publisher.publish(&snapshot, &control)', [System.StringComparison]::Ordinal)
if ($sourcePollIndex -lt 0 -or $publisherIndex -lt 0 -or $sourcePollIndex -ge $publisherIndex) {
    throw 'quota execution does not complete source poll before publisher admission'
}
$leaseIndex = $executionText.IndexOf('.lease', [System.StringComparison]::Ordinal)
$acquireIndex = $executionText.IndexOf('.try_acquire()', $leaseIndex, [System.StringComparison]::Ordinal)
$storeOpenIndex = $executionText.IndexOf('UsageStore::open(&self.archive_path)', [System.StringComparison]::Ordinal)
$applyIndex = $executionText.IndexOf('.apply_quota_observation(', [System.StringComparison]::Ordinal)
if (
    $leaseIndex -lt 0 -or
    $acquireIndex -lt 0 -or
    $storeOpenIndex -lt 0 -or
    $applyIndex -lt 0 -or
    $acquireIndex -ge $storeOpenIndex -or
    $storeOpenIndex -ge $applyIndex
) {
    throw 'quota publisher does not acquire lease before bounded store publication'
}
$requiredExecutionPatterns = @(
    'MAX_CODEX_QUOTA_WINDOWS',
    'for observation in snapshot\.observations\(\)',
    'control\.check\(\)',
    'QuotaApplyStatus::Duplicate',
    'QuotaApplyStatus::Stale',
    'CodexQuotaRetryMode::Accelerated',
    'CodexQuotaErrorCode::UnsupportedVersion'
)
foreach ($pattern in $requiredExecutionPatterns) {
    if ($executionText -notmatch $pattern) {
        throw "quota execution is missing required bounded behavior: $pattern"
    }
}

$requiredRuntimePatterns = @(
    'RefreshWorker::spawn',
    'RefreshScheduler::spawn_paused',
    'CodexQuotaRuntimePhase::Paused',
    'PowerLifecycleEvent::Suspend',
    'PowerLifecycleEvent::Resume',
    'Arc::get_mut\(&mut self\.worker\)',
    'CodexQuotaRetryMode::Accelerated => WatcherHealth::Degraded'
)
foreach ($pattern in $requiredRuntimePatterns) {
    if ($runtimeText -notmatch $pattern) {
        throw "quota runtime is missing required lifecycle behavior: $pattern"
    }
}
if ($runtimeText -match 'LiveRuntime|CodexAdapter|refresh_incremental') {
    throw 'quota runtime is coupled to usage runtime execution'
}
$forbiddenHealthPattern = @(
    'PathBuf|\bPath\b',
    'account_id|workspace_id|window_id|display_label',
    'used_ratio|remaining_ratio|quota_value',
    'email|credential|raw_frame|response_body'
) -join '|'
if ($healthText -match $forbiddenHealthPattern) {
    throw 'quota health contract contains path, identity, value, or raw-provider payload state'
}

$discoveryContract = Join-Path $root 'crates\runtime\tests\quota_discovery_contract.rs'
$discoveryContractText = [System.IO.File]::ReadAllText($discoveryContract)
foreach ($needle in @('"codex.cmd"', '"codex.ps1"', '"codex"', 'relative-entry')) {
    if ($discoveryContractText.IndexOf($needle, [System.StringComparison]::Ordinal) -lt 0) {
        throw "quota discovery contract is missing shim/path rejection vector: $needle"
    }
}

& cargo +1.97.0 build --release --locked --manifest-path $manifest -p tokenmaster-runtime --lib
if ($LASTEXITCODE -ne 0) {
    throw 'release tokenmaster-runtime library build failed'
}
$targetDirectory = [System.IO.Path]::GetFullPath([string]$metadata.target_directory)
$artifact = Get-ChildItem -LiteralPath $targetDirectory -Recurse -File |
    Where-Object {
        ($_.FullName -match '[\\/]release[\\/]deps[\\/]') -and
        ($_.Name -match '^(lib)?tokenmaster_runtime-.*\.rlib$')
    } |
    Sort-Object LastWriteTimeUtc -Descending |
    Select-Object -First 1
if ($null -eq $artifact) {
    throw 'release tokenmaster-runtime library artifact was not found'
}

$bytes = [System.IO.File]::ReadAllBytes($artifact.FullName)
$artifactText = [System.Text.Encoding]::ASCII.GetString($bytes)
$forbiddenBinaryStrings = @(
    'http://', 'https://', 'Set-Cookie', 'cookie_store', 'auth.json',
    'private endpoint', 'Authorization: Bearer', 'powershell.exe', 'cmd.exe',
    'TcpStream', 'TcpListener', 'UdpSocket'
)
foreach ($needle in $forbiddenBinaryStrings) {
    if ($artifactText.IndexOf($needle, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "release quota runtime contains forbidden authority string: $needle"
    }
}

[ordered]@{
    result = 'pass'
    package = 'tokenmaster-runtime'
    dependency_count = $dependencyNames.Count
    forbidden_dependency_count = 0
    production_quota_source_file_count = $librarySourceFiles.Count
    foreign_runtime_file_count = 0
    exact_native_discovery = $true
    source_before_publisher = $true
    lease_before_store = $true
    separate_usage_runtime = $true
    release_artifact_count = 1
    forbidden_binary_string_count = 0
} | ConvertTo-Json -Compress
