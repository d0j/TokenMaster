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
$packagesById = @{}
foreach ($package in $metadata.packages) {
    $packagesById[$package.id] = $package
}
$nodesById = @{}
foreach ($node in $metadata.resolve.nodes) {
    $nodesById[$node.id] = $node
}
$rootNames = @('tokenmaster-git', 'tokenmaster-store', 'tokenmaster-query', 'tokenmaster-runtime')
$rootPackages = @($metadata.packages | Where-Object { $_.name -in $rootNames })
if ($rootPackages.Count -ne $rootNames.Count) {
    throw 'Git production packages were not resolved exactly once'
}

$visited = [System.Collections.Generic.HashSet[string]]::new()
$pending = [System.Collections.Generic.Queue[string]]::new()
foreach ($package in $rootPackages) {
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
$dependencyNames = @(
    $visited | ForEach-Object { $packagesById[$_].name } | Sort-Object -Unique
)
$forbiddenCrates = @(
    'reqwest', 'ureq', 'hyper', 'hyper-util', 'h2', 'curl', 'isahc', 'surf',
    'awc', 'tokio', 'async-std', 'smol', 'webbrowser', 'headless_chrome',
    'chromiumoxide', 'thirtyfour', 'fantoccini', 'cookie_store', 'git2', 'gix'
)
$forbiddenDependencies = @($dependencyNames | Where-Object { $_ -in $forbiddenCrates })
if ($forbiddenDependencies.Count -ne 0) {
    throw "Git dependency closure contains forbidden authority crates: $($forbiddenDependencies -join ', ')"
}

$gitRoot = Join-Path $root 'crates\git\src'
$runtimeRoot = Join-Path $root 'crates\runtime\src\git'
$storeRoot = Join-Path $root 'crates\store\src\usage'
$queryPath = Join-Path $root 'crates\query\src\git_output.rs'
$gitFiles = @(Get-ChildItem -LiteralPath $gitRoot -Recurse -File)
$runtimeFiles = @(Get-ChildItem -LiteralPath $runtimeRoot -Recurse -File)
$storeFiles = @(Get-ChildItem -LiteralPath $storeRoot -File -Filter 'git_*.rs')
$queryFiles = @(Get-Item -LiteralPath $queryPath)
$boundaryFiles = @($gitFiles + $runtimeFiles + $storeFiles + $queryFiles)
if (@($gitFiles + $runtimeFiles | Where-Object { $_.Extension -ne '.rs' }).Count -ne 0) {
    throw 'Git or Git runtime production source contains a foreign-language file'
}

$productionText = ($boundaryFiles | ForEach-Object {
    Get-ProductionRustText -Path $_.FullName
}) -join "`n"
$forbiddenAuthorityPattern = @(
    'https?://',
    '\bstd::net\b|\bTcp(Stream|Listener)\b|\bUdpSocket\b',
    '\b(reqwest|ureq|isahc|fantoccini|chromiumoxide|thirtyfour)\b',
    'headless[_-]?chrome|webbrowser|cookie[_-]?store|set-cookie',
    'auth\.json|[\\/]\.codex[\\/]auth',
    '\bAuthorization\b|\bBearer\s',
    'powershell(?:\.exe)?|cmd(?:\.exe)?|bash(?:\.exe)?|\bsh\s+-c\b'
) -join '|'
if ($productionText -match $forbiddenAuthorityPattern) {
    throw 'Git production boundary contains network/browser/credential/shell authority'
}

$commandPath = Join-Path $gitRoot 'command.rs'
$scanPath = Join-Path $gitRoot 'scan.rs'
$processPath = Join-Path $gitRoot 'process.rs'
$executionPath = Join-Path $runtimeRoot 'execution.rs'
$runtimePath = Join-Path $runtimeRoot 'runtime.rs'
$healthPath = Join-Path $runtimeRoot 'health.rs'
$configPath = Join-Path $runtimeRoot 'config.rs'
$commandText = Get-ProductionRustText -Path $commandPath
$scanText = Get-ProductionRustText -Path $scanPath
$processText = Get-ProductionRustText -Path $processPath
$executionText = Get-ProductionRustText -Path $executionPath
$runtimeText = Get-ProductionRustText -Path $runtimePath
$healthText = Get-ProductionRustText -Path $healthPath
$configText = Get-ProductionRustText -Path $configPath
$storeText = ($storeFiles | ForEach-Object { Get-ProductionRustText -Path $_.FullName }) -join "`n"
$queryText = Get-ProductionRustText -Path $queryPath

foreach ($pattern in @(
    'Command::new\(&self\.path\)',
    'validate_native_name',
    'GIT_OPTIONAL_LOCKS", "0',
    'GIT_TERMINAL_PROMPT", "0',
    'GIT_CONFIG_NOSYSTEM", "1',
    'GIT_PROTOCOL_FROM_USER", "0',
    'GIT_EXTERNAL_DIFF',
    'GIT_SSH_COMMAND',
    'GitExecutable\(\[redacted\]\)',
    'GitRepositoryCandidate\(\[redacted\]\)'
)) {
    if ($commandText -notmatch $pattern) {
        throw "Git command boundary is missing required isolation: $pattern"
    }
}
foreach ($pattern in @(
    '"rev-parse"',
    '"for-each-ref"',
    '"--no-ext-diff"',
    '"--no-textconv"',
    '"--use-mailmap"',
    '"merge-base"',
    '"--is-ancestor"',
    'MAX_INCREMENTAL_ARGUMENT_BYTES',
    'HistoryChangedDuringScan',
    'derive_repository_id'
)) {
    if ($scanText -notmatch $pattern) {
        throw "Git scan boundary is missing required fixed behavior: $pattern"
    }
}
if ($scanText -match '"(add|commit|reset|checkout|switch|restore|fetch|pull|push|clone|gc|clean|rebase|merge)"') {
    throw 'Git production scan contains a repository mutation or network command'
}
foreach ($pattern in @(
    'MAX_GIT_LOG_STDOUT_BYTES',
    'MAX_GIT_STDERR_BYTES',
    'operation_deadline',
    'stop_and_reap',
    'is_cancelled'
)) {
    if ($processText -notmatch $pattern) {
        throw "Git process boundary is missing a required bound or cleanup path: $pattern"
    }
}

$scanIndex = $executionText.IndexOf('process.refresh(', [System.StringComparison]::Ordinal)
$leaseIndex = $executionText.IndexOf('.try_acquire()', [System.StringComparison]::Ordinal)
$storeOpenIndex = $executionText.IndexOf('UsageStore::open(self.config.archive_path())', [System.StringComparison]::Ordinal)
if (
    $scanIndex -lt 0 -or
    $leaseIndex -lt 0 -or
    $storeOpenIndex -lt 0 -or
    $scanIndex -ge $leaseIndex -or
    $leaseIndex -ge $storeOpenIndex
) {
    throw 'Git runtime does not preserve complete Git I/O before lease and store open'
}
foreach ($pattern in @(
    'GitCancellation::linked',
    'GitRefreshKind::Unchanged',
    'GitRefreshKind::Append',
    'GitRefreshKind::Rebuild',
    'GitOutputQuality::Unavailable',
    'mark_git_rebuild_required',
    'derive_activity_association_id'
)) {
    if ($executionText -notmatch $pattern) {
        throw "Git runtime publication is missing required bounded truth: $pattern"
    }
}
foreach ($pattern in @(
    'MAX_GIT_RUNTIME_REPOSITORIES',
    'VecDeque::with_capacity\(MAX_GIT_RUNTIME_REPOSITORIES\)',
    'hints\.slots\[index\]\.frontier = None',
    'force_reconcile\(RefreshUrgency::Recovery\)',
    'self\.worker\.cancel\(active\)',
    'Arc::get_mut\(&mut self\.worker\)',
    'hints\.slots\.clear\(\)'
)) {
    if ($runtimeText -notmatch $pattern) {
        throw "Git runtime lifecycle is missing required bounded cleanup/recovery: $pattern"
    }
}
if ($healthText -match 'PathBuf|\bPath\b|RepositoryCandidate|repository_id|association_id|project_key|email|stderr|raw_output') {
    throw 'Git health exposes path, identity, project, author, or raw output state'
}
if ($configText -notmatch '\.field\("archive_path", &"\[redacted\]"\)') {
    throw 'Git runtime config Debug does not redact the archive path'
}
if ($runtimeText -notmatch 'GitRepositoryHintIngress\(\[redacted\]\)') {
    throw 'Git hint ingress Debug does not redact transient candidates'
}
if ($queryText -match 'PathBuf|std::path|RepositoryCandidate|GitRefHead|GitAuthorFingerprint|stderr|raw_output') {
    throw 'Git public query exposes a raw path, ref, author, stderr, or raw output type'
}
if ($storeText -match 'PathBuf|std::path|author_email|raw_output|stderr|commit_id|object_id|ref_name') {
    throw 'Git durable store contains a raw path, author, output, commit, object, or ref field'
}
if ($executionText -match '\brusqlite\b|transaction_with_behavior|\bSELECT\b|\bINSERT\b|\bUPDATE\b|\bDELETE\s+FROM\b') {
    throw 'Git runtime contains direct SQL authority'
}

$contractFiles = @(
    Get-ChildItem -LiteralPath (Join-Path $root 'crates\git\tests') -File -Filter '*.rs'
    Get-ChildItem -LiteralPath (Join-Path $root 'crates\store\tests') -File -Filter 'git_*.rs'
    Get-ChildItem -LiteralPath (Join-Path $root 'crates\query\tests') -File -Filter 'git_*.rs'
    Get-ChildItem -LiteralPath (Join-Path $root 'crates\runtime\tests') -File -Filter 'git_runtime*.rs'
)
$contractText = ($contractFiles | ForEach-Object { [System.IO.File]::ReadAllText($_.FullName) }) -join "`n"
foreach ($needle in @(
    'synthetic_history_has_exact_author_merge_rename_binary_submodule_and_branch_semantics',
    'octopus_merge_is_counted_once_without_recounting_merged_lines',
    'worktrees_share_repository_identity_and_empty_repository_is_complete_zero',
    'shallow_boundary_is_explicit_and_never_fabricates_hidden_history',
    'missing_author_is_explicit_and_repository_paths_never_enter_arguments_or_errors',
    'unchanged_skips_history_and_append_scans_only_new_reachable_commits',
    'rewritten_history_falls_back_to_one_authoritative_rebuild',
    'failed_rebuild_preserves_prior_publication_and_generation',
    'git_output_is_immutable_utc_bounded_and_private',
    'identified_repository_without_author_publishes_explicit_unavailable_truth',
    'pause_cancels_and_reaps_the_active_git_child_before_returning',
    'writer_contention_happens_after_git_io_and_before_publication',
    'superseded_hint_rejects_the_scanned_result_and_runs_one_follow_up'
)) {
    if ($contractText.IndexOf($needle, [System.StringComparison]::Ordinal) -lt 0) {
        throw "Git acceptance contract is missing vector: $needle"
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

& cargo +1.97.0 build --release --locked --manifest-path $manifest `
    -p tokenmaster-git -p tokenmaster-store -p tokenmaster-query -p tokenmaster-runtime
if ($LASTEXITCODE -ne 0) {
    throw 'release Git production libraries build failed'
}
$targetDirectory = [System.IO.Path]::GetFullPath([string]$metadata.target_directory)
$artifactNames = @('git', 'store', 'query', 'runtime')
$artifacts = @()
foreach ($name in $artifactNames) {
    $artifact = Get-ChildItem -LiteralPath $targetDirectory -Recurse -File |
        Where-Object {
            ($_.FullName -match '[\\/]release[\\/]deps[\\/]') -and
            ($_.Name -match "^(lib)?tokenmaster_$name-.*\.rlib$")
        } |
        Sort-Object LastWriteTimeUtc -Descending |
        Select-Object -First 1
    if ($null -eq $artifact) {
        throw "release artifact was not found for tokenmaster-$name"
    }
    $artifacts += $artifact
}
$forbiddenBinaryStrings = @(
    'PRIVATE_GIT_RUNTIME_REPOSITORY',
    'private-runtime@example.com',
    'runtime@example.com',
    'C:\private\git',
    'Authorization: Bearer',
    'auth.json',
    'powershell.exe',
    'cmd.exe',
    'git add',
    'git commit',
    'git push',
    'git fetch'
)
foreach ($artifact in $artifacts) {
    $bytes = [System.IO.File]::ReadAllBytes($artifact.FullName)
    $artifactText = [System.Text.Encoding]::ASCII.GetString($bytes)
    foreach ($needle in $forbiddenBinaryStrings) {
        if ($artifactText.IndexOf($needle, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "release Git artifact contains forbidden private/authority string: $needle"
        }
    }
}

[ordered]@{
    result = 'pass'
    packages = $rootNames
    dependency_count = $dependencyNames.Count
    forbidden_dependency_count = 0
    production_boundary_file_count = $boundaryFiles.Count
    foreign_git_source_file_count = 0
    git_io_before_lease = $true
    lease_before_store = $true
    runtime_direct_sql = $false
    exact_native_git = $true
    mutation_command_count = 0
    transient_frontier = $true
    count_only_health = $true
    vendored_upstream_source_count = 0
    release_artifact_count = $artifacts.Count
    forbidden_binary_string_count = 0
} | ConvertTo-Json -Compress
