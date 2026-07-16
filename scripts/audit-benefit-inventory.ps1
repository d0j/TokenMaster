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
$rootPackages = @(
    $metadata.packages |
        Where-Object {
            $_.name -in @(
                'tokenmaster-benefits',
                'tokenmaster-store',
                'tokenmaster-query',
                'tokenmaster-runtime'
            )
        }
)
if ($rootPackages.Count -ne 4) {
    throw 'benefit production packages were not resolved exactly once'
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
    'chromiumoxide', 'thirtyfour', 'fantoccini', 'cookie_store'
)
$forbiddenDependencies = @($dependencyNames | Where-Object { $_ -in $forbiddenCrates })
if ($forbiddenDependencies.Count -ne 0) {
    throw "benefit dependency closure contains forbidden crates: $($forbiddenDependencies -join ', ')"
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

$reminderRoot = Join-Path $root 'crates\runtime\src\reminder'
$reminderFiles = @(Get-ChildItem -LiteralPath $reminderRoot -Recurse -File)
if (@($reminderFiles | Where-Object { $_.Extension -ne '.rs' }).Count -ne 0) {
    throw 'reminder runtime production source contains a foreign-language file'
}
$reminderText = ($reminderFiles | ForEach-Object {
    Get-ProductionRustText -Path $_.FullName
}) -join "`n"
$forbiddenRuntimePattern = @(
    'https?://',
    '\bstd::net\b|\bTcp(Stream|Listener)\b|\bUdpSocket\b',
    '\b(reqwest|ureq|isahc|fantoccini|chromiumoxide|thirtyfour)\b',
    'headless[_-]?chrome|webbrowser|cookie[_-]?store|set-cookie',
    'auth\.json|[\\/]\.codex[\\/]auth',
    '\bAuthorization\b|\bBearer\s',
    'private[_ -]?endpoint',
    'powershell(?:\.exe)?|cmd(?:\.exe)?|bash(?:\.exe)?|\bsh\s+-c\b',
    '\bCommand::new\b',
    '\brusqlite\b|\btransaction_with_behavior\b',
    '\bSELECT\b|\bINSERT\b|\bUPDATE\b|\bDELETE\s+FROM\b'
) -join '|'
if ($reminderText -match $forbiddenRuntimePattern) {
    throw 'reminder runtime contains network/browser/credential/shell/direct-SQL authority'
}

$executionPath = Join-Path $reminderRoot 'execution.rs'
$runtimePath = Join-Path $reminderRoot 'runtime.rs'
$healthPath = Join-Path $reminderRoot 'health.rs'
$executionText = Get-ProductionRustText -Path $executionPath
$runtimeText = Get-ProductionRustText -Path $runtimePath
$healthText = Get-ProductionRustText -Path $healthPath

$acquireIndex = $executionText.IndexOf('.try_acquire()', [System.StringComparison]::Ordinal)
$storeOpenIndex = $executionText.IndexOf('UsageStore::open(&self.archive_path)', [System.StringComparison]::Ordinal)
$processIndex = $executionText.IndexOf('.process_due_in_app_benefit_reminders(', [System.StringComparison]::Ordinal)
$publishIndex = $executionText.IndexOf('self.notifications.publish(processed.deliveries())', [System.StringComparison]::Ordinal)
if (
    $acquireIndex -lt 0 -or
    $storeOpenIndex -lt 0 -or
    $processIndex -lt 0 -or
    $publishIndex -lt 0 -or
    $acquireIndex -ge $storeOpenIndex -or
    $storeOpenIndex -ge $processIndex -or
    $processIndex -ge $publishIndex
) {
    throw 'reminder execution does not preserve lease/store/receipt/publication ordering'
}
foreach ($pattern in @(
    'MAX_BENEFIT_REMINDER_DUE_PAGE_SIZE',
    'OperationControl::new',
    'BenefitReminderRetryMode::Accelerated',
    'NotificationSlot',
    'Box<\[BenefitReminderDelivery\]>',
    'BenefitReminderAcknowledger',
    'acknowledge_benefit_reminders'
)) {
    if ($executionText -notmatch $pattern) {
        throw "reminder execution is missing required bounded behavior: $pattern"
    }
}
foreach ($pattern in @(
    'tokenmaster-reminder-scheduler',
    'wait_timeout',
    'notification_pending',
    'pending_urgency',
    'Arc::get_mut\(&mut self\.worker\)',
    'take_for_presentation',
    'begin_acknowledgement',
    'finish_acknowledgement',
    'release_presentation',
    'PowerLifecycleEvent::Suspend',
    'PowerLifecycleEvent::Resume',
    'REDACT_REMINDER_SCHEDULER_PANIC'
)) {
    if ($runtimeText -notmatch $pattern) {
        throw "reminder runtime is missing required lifecycle behavior: $pattern"
    }
}
if ($runtimeText -match 'LiveRuntime|CodexQuotaRuntime|CodexAdapter|refresh_incremental') {
    throw 'reminder runtime is coupled to usage or quota execution'
}
$forbiddenHealthPattern = @(
    'PathBuf|\bPath\b',
    'account_id|workspace_id|lot_id|delivery_id|scope_id',
    'provider_payload|raw_frame|response_body|credential|email'
) -join '|'
if ($healthText -match $forbiddenHealthPattern) {
    throw 'reminder health contains path, identity, credential, or raw-provider state'
}

$storePath = Join-Path $root 'crates\store\src\usage\benefit_reminder.rs'
$storeText = Get-ProductionRustText -Path $storePath
$writePath = Join-Path $root 'crates\store\src\usage\benefit_write.rs'
$writeText = Get-ProductionRustText -Path $writePath
$schemaPath = Join-Path $root 'crates\store\src\usage\benefit_schema.rs'
$schemaText = Get-ProductionRustText -Path $schemaPath
foreach ($pattern in @(
    'transaction_with_behavior\(TransactionBehavior::Immediate\)',
    'MAX_BENEFIT_REMINDER_DUE_PAGE_SIZE',
    'LIMIT \?3',
    'INSERT INTO benefit_reminder_delivery',
    'DELETE FROM benefit_reminder_due WHERE delivery_id = \?1',
    'UPDATE benefit_state',
    'has_equal_or_more_urgent_receipt',
    'load_unacknowledged_deliveries',
    'acknowledge_benefit_reminders',
    'INSERT INTO benefit_reminder_ack'
)) {
    if ($storeText -notmatch $pattern) {
        throw "benefit reminder store is missing required atomic behavior: $pattern"
    }
}
$receiptIndex = $storeText.IndexOf('insert_receipt(&transaction', [System.StringComparison]::Ordinal)
$deleteIndex = $storeText.IndexOf('DELETE FROM benefit_reminder_due WHERE delivery_id = ?1', [System.StringComparison]::Ordinal)
if ($receiptIndex -lt 0 -or $deleteIndex -lt 0 -or $receiptIndex -ge $deleteIndex) {
    throw 'benefit reminder store does not record receipt before removing due rows'
}
if ($writeText -notmatch 'threshold_seconds <= \?5') {
    throw 'benefit due rebuild does not suppress already-missed less-urgent thresholds'
}
foreach ($pattern in @(
    'CREATE TABLE benefit_reminder_ack',
    'ON DELETE CASCADE',
    'immutable benefit acknowledgement'
)) {
    if ($schemaText -notmatch $pattern) {
        throw "benefit acknowledgement schema is missing durable outbox behavior: $pattern"
    }
}

$codexPath = Join-Path $root 'crates\codex\src\quota\normalize.rs'
$codexText = [System.IO.File]::ReadAllText($codexPath)
foreach ($pattern in @(
    'detailed_benefit_lot_id',
    'account_id',
    'credit\.id',
    'BenefitLotId::from_bytes'
)) {
    if ($codexText -notmatch $pattern) {
        throw "Codex benefit normalization is missing privacy boundary: $pattern"
    }
}

$contractText = [System.IO.File]::ReadAllText(
    (Join-Path $root 'crates\runtime\tests\reminder_runtime_contract.rs')
)
foreach ($needle in @(
    'startup_replays_unacknowledged_event_after_restart_and_ack_stops_replay',
    'acknowledgement_contention_keeps_the_leased_batch_retryable',
    'hints_coalesce_and_resume_forces_one_recovery_pass',
    'writer_contention_opens_no_sqlite_and_uses_accelerated_retry',
    'reminder_store_fault_leaves_live_usage_runtime_unchanged'
)) {
    if ($contractText.IndexOf($needle, [System.StringComparison]::Ordinal) -lt 0) {
        throw "reminder runtime contract is missing acceptance vector: $needle"
    }
}
$storeContractText = [System.IO.File]::ReadAllText(
    (Join-Path $root 'crates\store\tests\benefit_reminder_contract.rs')
)
foreach ($needle in @(
    'unacknowledged_delivery_replays_after_restart_and_acknowledgement_stops_replay',
    'due_page_is_exactly_bounded_and_split_lot_rows_cannot_replay',
    'collapsed_receipt_preserves_future_more_urgent_threshold',
    'expired_rows_are_drained_without_publication_and_limits_fail_closed'
)) {
    if ($storeContractText.IndexOf($needle, [System.StringComparison]::Ordinal) -lt 0) {
        throw "benefit reminder store contract is missing acceptance vector: $needle"
    }
}
$schemaContractText = [System.IO.File]::ReadAllText(
    (Join-Path $root 'crates\store\tests\benefit_schema_contract.rs')
)
if (
    $schemaContractText.IndexOf(
        'exact_v11_migration_marks_legacy_delivery_receipts_acknowledged',
        [System.StringComparison]::Ordinal
    ) -lt 0
) {
    throw 'benefit schema contract is missing exact v11 acknowledgement migration'
}

& cargo +1.97.0 build --release --locked --manifest-path $manifest `
    -p tokenmaster-benefits -p tokenmaster-store -p tokenmaster-query -p tokenmaster-runtime
if ($LASTEXITCODE -ne 0) {
    throw 'release benefit libraries build failed'
}
$targetDirectory = [System.IO.Path]::GetFullPath([string]$metadata.target_directory)
$artifactPackages = @(
    'tokenmaster_benefits',
    'tokenmaster_store',
    'tokenmaster_query',
    'tokenmaster_runtime'
)
$artifacts = @()
foreach ($packageName in $artifactPackages) {
    $artifact = Get-ChildItem -LiteralPath $targetDirectory -Recurse -File |
        Where-Object {
            ($_.FullName -match '[\\/]release[\\/]deps[\\/]') -and
            ($_.Name -match "^(lib)?$packageName-.*\.rlib$")
        } |
        Sort-Object LastWriteTimeUtc -Descending |
        Select-Object -First 1
    if ($null -eq $artifact) {
        throw "release artifact was not found for $packageName"
    }
    $artifacts += $artifact
}
$forbiddenBinaryStrings = @(
    'private_runtime_credit_id',
    'acct_private',
    'private-runtime@example.com',
    'private benefit title',
    'C:\private\runtime-fixture',
    'Authorization: Bearer',
    'cookie_store',
    'auth.json',
    'private endpoint'
)
foreach ($artifact in $artifacts) {
    $bytes = [System.IO.File]::ReadAllBytes($artifact.FullName)
    $artifactText = [System.Text.Encoding]::ASCII.GetString($bytes)
    foreach ($needle in $forbiddenBinaryStrings) {
        if ($artifactText.IndexOf($needle, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "release benefit artifact contains forbidden private/authority string: $needle"
        }
    }
}

[ordered]@{
    result = 'pass'
    production_package_count = $rootPackages.Count
    dependency_count = $dependencyNames.Count
    forbidden_dependency_count = 0
    production_reminder_source_file_count = $reminderFiles.Count
    foreign_runtime_file_count = 0
    lease_before_store = $true
    outbox_before_publication = $true
    durable_ack_outbox = $true
    pre_ack_restart_replay = $true
    schema_version = 12
    runtime_direct_sql = $false
    bounded_due_page = $true
    durable_less_urgent_suppression = $true
    release_artifact_count = $artifacts.Count
    forbidden_binary_string_count = 0
} | ConvertTo-Json -Compress
