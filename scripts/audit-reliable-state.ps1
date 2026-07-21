[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$RepositoryRoot,
    [switch]$SourceOnly
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$rootManifest = Join-Path $root 'Cargo.toml'
$stateRoot = Join-Path $root 'crates\state'
$stateManifest = Join-Path $stateRoot 'Cargo.toml'
$stateSource = Join-Path $stateRoot 'src'
$faultMatrix = Join-Path $stateRoot 'tests\fault_matrix_contract.rs'

foreach ($required in @($rootManifest, $stateManifest, $stateSource, $faultMatrix)) {
    if (-not (Test-Path -LiteralPath $required)) {
        throw "TM-STATE-MISSING-BOUNDARY: $([System.IO.Path]::GetFileName($required))"
    }
}
$faultMatrixText = [System.IO.File]::ReadAllText($faultMatrix)
$faultMatrixAnchors = @(
    'fn every_package_prefix_and_one_bit_mutation_fails_closed()',
    'fn preexisting_wal_and_shm_drift_fails_before_any_archive_move()',
    'fn prepared_resume_completes_an_exact_partially_moved_sidecar_set()',
    'fn conflicting_resumed_sidecar_target_fails_before_any_active_move()'
)
foreach ($anchor in $faultMatrixAnchors) {
    if ([regex]::Matches($faultMatrixText, [regex]::Escape($anchor)).Count -ne 1) {
        throw "TM-STATE-FAULT-MATRIX: missing exact anchor $anchor"
    }
}

$rootManifestText = [System.IO.File]::ReadAllText($rootManifest)
$workspaceSection = [regex]::Match(
    $rootManifestText,
    '(?ms)^\s*\[workspace\]\s*$\s*(?<body>.*?)(?=^\s*\[|\z)'
)
$workspaceMembers = if ($workspaceSection.Success) {
    [regex]::Match(
        $workspaceSection.Groups['body'].Value,
        '(?ms)^\s*members\s*=\s*\[(?<items>.*?)\]'
    )
} else {
    [System.Text.RegularExpressions.Match]::Empty
}
$workspaceMemberNames = if ($workspaceMembers.Success) {
    $memberItems = [regex]::Replace(
        $workspaceMembers.Groups['items'].Value,
        '(?m)#.*$',
        ''
    )
    @(
        [regex]::Matches($memberItems, '"(?<member>[^"\r\n]+)"') |
            ForEach-Object { $_.Groups['member'].Value }
    )
} else {
    @()
}
$stateWorkspaceMembers = @($workspaceMemberNames | Where-Object { $_ -eq 'crates/state' })
if ($stateWorkspaceMembers.Count -ne 1) {
    throw 'TM-STATE-WORKSPACE: crates/state must be one exact workspace member'
}

$manifestText = [System.IO.File]::ReadAllText($stateManifest)
if ($manifestText -notmatch '(?m)^name\s*=\s*"tokenmaster-state"\s*$') {
    throw 'TM-STATE-PACKAGE: package identity must be tokenmaster-state'
}

$mainSource = Join-Path $stateSource 'main.rs'
$binarySource = Join-Path $stateSource 'bin'
if ($manifestText -match '\[\[bin\]\]' -or
    $manifestText -match '(?m)^\s*autobins\s*=\s*true\s*$' -or
    (Test-Path -LiteralPath $mainSource) -or
    (Test-Path -LiteralPath $binarySource)) {
    throw 'TM-STATE-BINARY-TARGET: tokenmaster-state must remain library-only'
}
if ($manifestText -match '(?m)^\s*build\s*=' -or
    (Test-Path -LiteralPath (Join-Path $stateRoot 'build.rs'))) {
    throw 'TM-STATE-BUILD-SCRIPT: tokenmaster-state must not own a build script'
}

$dependencyNames = [System.Collections.Generic.List[string]]::new()
$insideDependencies = $false
foreach ($line in ($manifestText -split "`r?`n")) {
    if ($line -match '^\s*\[dependencies\]\s*$') {
        $insideDependencies = $true
        continue
    }
    if ($line -match '^\s*\[') {
        $insideDependencies = $false
        continue
    }
    if ($insideDependencies -and $line -match '^\s*([A-Za-z0-9_-]+)(?:\.[A-Za-z0-9_-]+)?\s*=') {
        $dependencyNames.Add($Matches[1])
    }
}
$directProductionDependencies = @($dependencyNames | Sort-Object -Unique)
$expectedDependencies = @(
    'age', 'serde', 'serde_json', 'sha2', 'thiserror', 'tokenmaster-platform',
    'tokenmaster-store', 'zstd'
)
if ($directProductionDependencies.Count -ne $expectedDependencies.Count -or
    @($expectedDependencies | Where-Object { $_ -notin $directProductionDependencies }).Count -ne 0) {
    throw "TM-STATE-DEPENDENCIES: direct dependency set drifted: $($directProductionDependencies -join ', ')"
}
if (@([regex]::Matches(
            $manifestText,
            '(?m)^tokenmaster-store\s*=\s*\{\s*path\s*=\s*"\.\./store"\s*\}\s*$'
        )).Count -ne 1) {
    throw 'TM-STATE-STORE-PIN: store interop must resolve through the exact workspace path'
}
if ($rootManifestText -notmatch '(?m)^zstd\s*=\s*\{\s*version\s*=\s*"=0\.13\.3"\s*,\s*default-features\s*=\s*false\s*\}\s*$' -or
    $manifestText -notmatch '(?m)^zstd\.workspace\s*=\s*true\s*$') {
    throw 'TM-STATE-ZSTD-PIN: zstd must remain exactly 0.13.3 with default features disabled'
}
if ($rootManifestText -notmatch '(?m)^age\s*=\s*\{\s*version\s*=\s*"=0\.12\.1"\s*,\s*default-features\s*=\s*false\s*\}\s*$' -or
    $manifestText -notmatch '(?m)^age\.workspace\s*=\s*true\s*$') {
    throw 'TM-STATE-AGE-PIN: age must remain exactly 0.12.1 with default features disabled'
}

$testOnlySource = Join-Path $stateSource 'record_contract_tests.rs'
$librarySource = Join-Path $stateSource 'lib.rs'
if (-not (Test-Path -LiteralPath $testOnlySource)) {
    throw 'TM-STATE-TEST-BOUNDARY: record contract test module is missing'
}
$librarySourceText = [System.IO.File]::ReadAllText($librarySource)
$testModulePattern = '(?m)^#\[cfg\(test\)\]\s*\r?\nmod record_contract_tests;\s*$'
if (@([regex]::Matches($librarySourceText, $testModulePattern)).Count -ne 1 -or
    @([regex]::Matches($librarySourceText, '\brecord_contract_tests\b')).Count -ne 1) {
    throw 'TM-STATE-TEST-BOUNDARY: record contract code must remain one cfg(test)-only module'
}
$rustFiles = @(
    Get-ChildItem -LiteralPath $stateSource -Recurse -File -Filter '*.rs' |
        Where-Object { $_.FullName -ne $testOnlySource }
)
if ($rustFiles.Count -eq 0) {
    throw 'TM-STATE-SOURCE: tokenmaster-state has no Rust library source'
}
$productionText = ($rustFiles | ForEach-Object {
    [System.IO.File]::ReadAllText($_.FullName)
}) -join "`n"
$packageCapabilityText = [System.IO.File]::ReadAllText(
    (Join-Path $stateSource 'package\capability.rs')
)
$settingsStoreText = [System.IO.File]::ReadAllText(
    (Join-Path $stateSource 'settings\store.rs')
)
$settingsMigrationText = [System.IO.File]::ReadAllText(
    (Join-Path $stateSource 'settings\migration.rs')
)
$settingsValueText = [System.IO.File]::ReadAllText(
    (Join-Path $stateSource 'settings\value.rs')
)
$runStateText = [System.IO.File]::ReadAllText(
    (Join-Path $stateSource 'run_state.rs')
)
$bootstrapText = [System.IO.File]::ReadAllText(
    (Join-Path $stateSource 'bootstrap.rs')
)
$recoveryJournalText = [System.IO.File]::ReadAllText(
    (Join-Path $stateSource 'recovery\journal.rs')
)
$recoveryRestoreText = [System.IO.File]::ReadAllText(
    (Join-Path $stateSource 'recovery\restore.rs')
)

$approvedStdIoPattern = '(?m)^\s*use\s+std\s*::\s*io\s*::\s*\{\s*self\s*,\s*Write\s*,?\s*\}\s*;\s*$'
$approvedPackageReaderIoPattern = '(?m)^\s*use\s+std\s*::\s*io\s*::\s*\{\s*self\s*,\s*BufRead\s*,\s*Read\s*,\s*Write\s*,?\s*\}\s*;\s*$'
$approvedPackageWriterIoPattern = '(?m)^\s*use\s+std\s*::\s*io\s*::\s*\{\s*self\s*,\s*Read\s*,\s*Write\s*,?\s*\}\s*;\s*$'
$approvedPlatformPattern = '(?ms)^\s*use\s+tokenmaster_platform\s*::\s*\{\s*DurableFileError\s*,\s*DurableFileTarget\s*,\s*DurableStagedFile\s*,\s*MAX_DURABLE_WRITE_CHUNK_BYTES\s*,\s*ValidatedLocalDirectory\s*,?\s*\}\s*;\s*$'
$approvedPackageCapabilityExportPattern = '(?ms)^\s*pub\(crate\)\s+use\s+tokenmaster_platform\s*::\s*\{\s*BackupStagedFile\s*,\s*DurableFileReader\s*,\s*DurableStagedFile\s*,\s*RecoveryStagedArchive\s*,?\s*\}\s*;\s*$'
$approvedPackageCapabilityImportPattern = '(?ms)^\s*use\s+tokenmaster_platform\s*::\s*\{\s*ArchiveRecoveryError\s*,\s*BackupDirectoryError\s*,\s*DurableFileError\s*,\s*MAX_DURABLE_WRITE_CHUNK_BYTES\s*,?\s*\}\s*;\s*$'
$approvedSettingsPlatformPattern = '(?m)^\s*use\s+tokenmaster_platform\s*::\s*ValidatedLocalDirectory\s*;\s*$'
$approvedCatalogPlatformPattern = '(?ms)^\s*use\s+tokenmaster_platform\s*::\s*\{\s*BackupDirectory\s*,\s*BackupDirectoryEntry\s*,\s*BackupDirectoryError\s*,\s*BackupDirectoryGeneration\s*,\s*DurableFileReader\s*,\s*MAX_DURABLE_FILE_BYTES\s*,?\s*\}\s*;\s*$'
$approvedRetentionPlatformPattern = '(?m)^\s*use\s+tokenmaster_platform\s*::\s*\{\s*BackupDirectory\s*,\s*BackupDirectoryError\s*,\s*MAX_BACKUP_DIRECTORY_FILES\s*,?\s*\}\s*;\s*$'
$approvedRecoveryJournalPlatformPattern = '(?ms)^\s*use\s+tokenmaster_platform\s*::\s*\{\s*ArchiveRecoveryError\s*,\s*ArchiveSetExpectation\s*,\s*ArchiveSetObservation\s*,\s*RecoveryOperationId\s*,\s*ValidatedLocalDirectory\s*,?\s*\}\s*;\s*$'
$approvedRecoveryPlatformPattern = '(?ms)^\s*use\s+tokenmaster_platform\s*::\s*\{\s*ArchiveRecoveryError\s*,\s*ArchiveRecoveryScope\s*,\s*ArchiveSetObservation\s*,\s*BackupDirectory\s*,\s*BackupDirectoryError\s*,\s*DurableFileReader\s*,\s*ExclusiveFileLeaseGuard\s*,\s*RecoveryMainMode\s*,\s*RecoveryOperation\s*,\s*RecoveryStagedArchive\s*,\s*ValidatedLocalDirectory\s*,?\s*\}\s*;\s*$'
$approvedBootstrapPlatformPattern = '(?ms)^\s*use\s+tokenmaster_platform\s*::\s*\{\s*ArchiveRecoveryError\s*,\s*ArchiveRecoveryScope\s*,\s*BackupDirectory\s*,\s*BackupDirectoryError\s*,\s*ExclusiveFileLeaseGuard\s*,\s*ValidatedLocalDirectory\s*,?\s*\}\s*;\s*$'
$approvedStoreCandidatePattern = '(?m)^\s*use\s+tokenmaster_store\s*::\s*\{\s*StoreErrorCode\s*,\s*VerifiedBackupCandidateReader\s*,?\s*\}\s*;\s*$'
$approvedRecoveryStorePattern = '(?ms)^\s*use\s+tokenmaster_store\s*::\s*\{\s*BackupControl\s*,\s*BackupStaging\s*,\s*RecoveryVerificationBoundary\s*,\s*StoreErrorCode\s*,\s*VerifiedRecoveryArchive\s*,\s*create_fresh_recovery_archive\s*,\s*verify_recovery_archive_with_observer\s*,?\s*\}\s*;\s*$'
$approvedBootstrapStorePattern = '(?ms)^\s*use\s+tokenmaster_store\s*::\s*\{\s*BackupControl\s*,\s*BackupStaging\s*,\s*StartupArchiveStatus\s*,\s*StartupValidationMode\s*,\s*StoreErrorCode\s*,\s*inspect_startup_archive\s*,?\s*\}\s*;\s*$'
$approvedMaintenanceStoreControlPattern = '(?m)^use tokenmaster_store::BackupControl;\r?$'
$approvedMaintenanceCoordinatorStdPattern = '(?m)^use std::sync::Arc;\r?\nuse std::sync::atomic::\{AtomicBool, AtomicU8, Ordering\};\r?\nuse std::time::Duration;\r?$'
$approvedMaintenanceSchedulerStdPattern = '(?m)^use std::sync::mpsc::\{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel\};\r?\nuse std::sync::\{Arc, Mutex\};\r?\nuse std::thread::\{Builder, JoinHandle\};\r?\nuse std::time::\{Duration, Instant\};\r?$'
$approvedMaintenanceWorkerStdPattern = '(?m)^use std::panic::\{AssertUnwindSafe, catch_unwind, set_hook, take_hook\};\r?\nuse std::sync::mpsc::\{Receiver, SyncSender, TrySendError, sync_channel\};\r?\nuse std::sync::\{Arc, Condvar, Mutex, Once\};\r?\nuse std::thread::\{Builder, JoinHandle\};\r?\nuse std::time::\{Duration, Instant\};\r?$'
$approvedMaintenanceOwnerStdPattern = '(?m)^mod coordinator;\r?\nmod scheduler;\r?\nmod worker;\r?\n\r?\nuse core::fmt;\r?\nuse std::sync::\{Arc, Mutex\};\r?\nuse std::time::Duration;\r?$'
$approvedStdIoImports = @([regex]::Matches($productionText, $approvedStdIoPattern))
$approvedPackageReaderIoImports = @(
    [regex]::Matches($productionText, $approvedPackageReaderIoPattern)
)
$approvedPackageWriterIoImports = @(
    [regex]::Matches($productionText, $approvedPackageWriterIoPattern)
)
$approvedPlatformImports = @([regex]::Matches($productionText, $approvedPlatformPattern))
$approvedPackageCapabilityExports = @(
    [regex]::Matches($productionText, $approvedPackageCapabilityExportPattern)
)
$approvedPackageCapabilityImports = @(
    [regex]::Matches($productionText, $approvedPackageCapabilityImportPattern)
)
$approvedSettingsPlatformImports = @(
    [regex]::Matches($settingsStoreText, $approvedSettingsPlatformPattern)
)
$approvedRunStatePlatformImports = @(
    [regex]::Matches($runStateText, $approvedSettingsPlatformPattern)
)
$approvedCatalogPlatformImports = @(
    [regex]::Matches($productionText, $approvedCatalogPlatformPattern)
)
$approvedRetentionPlatformImports = @(
    [regex]::Matches($productionText, $approvedRetentionPlatformPattern)
)
$approvedRecoveryJournalPlatformImports = @(
    [regex]::Matches($productionText, $approvedRecoveryJournalPlatformPattern)
)
$approvedRecoveryPlatformImports = @(
    [regex]::Matches($productionText, $approvedRecoveryPlatformPattern)
)
$approvedBootstrapPlatformImports = @(
    [regex]::Matches($bootstrapText, $approvedBootstrapPlatformPattern)
)
$approvedStoreCandidateImports = @(
    [regex]::Matches($productionText, $approvedStoreCandidatePattern)
)
$approvedRecoveryStoreImports = @(
    [regex]::Matches($productionText, $approvedRecoveryStorePattern)
)
$approvedBootstrapStoreImports = @(
    [regex]::Matches($bootstrapText, $approvedBootstrapStorePattern)
)
$ownedPackageCapabilityExports = @(
    [regex]::Matches($packageCapabilityText, $approvedPackageCapabilityExportPattern)
)
$ownedPackageCapabilityImports = @(
    [regex]::Matches($packageCapabilityText, $approvedPackageCapabilityImportPattern)
)
$ownedRecoveryJournalPlatformImports = @(
    [regex]::Matches($recoveryJournalText, $approvedRecoveryJournalPlatformPattern)
)
$ownedRecoveryPlatformImports = @(
    [regex]::Matches($recoveryRestoreText, $approvedRecoveryPlatformPattern)
)
$ownedRecoveryStoreImports = @(
    [regex]::Matches($recoveryRestoreText, $approvedRecoveryStorePattern)
)
$approvedMaintenanceStoreControlImports = @(
    [regex]::Matches($productionText, $approvedMaintenanceStoreControlPattern)
)
$approvedMaintenanceCoordinatorStdImports = @(
    [regex]::Matches($productionText, $approvedMaintenanceCoordinatorStdPattern)
)
$approvedMaintenanceSchedulerStdImports = @(
    [regex]::Matches($productionText, $approvedMaintenanceSchedulerStdPattern)
)
$approvedMaintenanceWorkerStdImports = @(
    [regex]::Matches($productionText, $approvedMaintenanceWorkerStdPattern)
)
$approvedMaintenanceOwnerStdImports = @(
    [regex]::Matches($productionText, $approvedMaintenanceOwnerStdPattern)
)
if ($approvedStdIoImports.Count -ne 1 -or
    $approvedPackageReaderIoImports.Count -ne 1 -or
    $approvedPackageWriterIoImports.Count -ne 3 -or
    $approvedPlatformImports.Count -ne 1 -or
    $approvedPackageCapabilityExports.Count -ne 1 -or
    $approvedPackageCapabilityImports.Count -ne 1 -or
    $ownedPackageCapabilityExports.Count -ne 1 -or
    $ownedPackageCapabilityImports.Count -ne 1 -or
    $approvedSettingsPlatformImports.Count -ne 1 -or
    $approvedRunStatePlatformImports.Count -ne 1 -or
    $approvedCatalogPlatformImports.Count -ne 1 -or
    $approvedRetentionPlatformImports.Count -ne 1 -or
    $approvedRecoveryJournalPlatformImports.Count -ne 1 -or
    $approvedRecoveryPlatformImports.Count -ne 1 -or
    $approvedBootstrapPlatformImports.Count -ne 1 -or
    $ownedRecoveryJournalPlatformImports.Count -ne 1 -or
    $ownedRecoveryPlatformImports.Count -ne 1 -or
    $approvedStoreCandidateImports.Count -ne 1 -or
    $approvedRecoveryStoreImports.Count -ne 1 -or
    $ownedRecoveryStoreImports.Count -ne 1 -or
    $approvedBootstrapStoreImports.Count -ne 1 -or
    $approvedMaintenanceStoreControlImports.Count -ne 1 -or
    $approvedMaintenanceCoordinatorStdImports.Count -ne 1 -or
    $approvedMaintenanceSchedulerStdImports.Count -ne 1 -or
    $approvedMaintenanceWorkerStdImports.Count -ne 1 -or
    $approvedMaintenanceOwnerStdImports.Count -ne 1) {
    throw 'TM-STATE-APPROVED-IO: exact bounded record/package capability imports must match the fixed allowlist'
}
$backupControlUses = @([regex]::Matches($productionText, '\bBackupControl\b'))
$verifiedCandidateReaderUses = @(
    [regex]::Matches($productionText, '\bVerifiedBackupCandidateReader\b')
)
if ($backupControlUses.Count -ne 21 -or $verifiedCandidateReaderUses.Count -ne 4) {
    throw 'TM-STATE-STORE-AUTHORITY: exact store capability use count drifted'
}
$sealedRecoveryCapabilityCounts = [ordered]@{
    BackupStaging = 6
    RecoveryVerificationBoundary = 3
    VerifiedRecoveryArchive = 4
    verify_recovery_archive_with_observer = 3
    create_fresh_recovery_archive = 2
    ArchiveRecoveryScope = 6
    ExclusiveFileLeaseGuard = 15
    RecoveryOperation = 2
    RecoveryStagedArchive = 9
    ArchiveSetExpectation = 3
    ArchiveSetObservation = 4
    RecoveryOperationId = 5
}
foreach ($capability in $sealedRecoveryCapabilityCounts.Keys) {
    $actual = @([regex]::Matches($productionText, "\b$capability\b")).Count
    if ($actual -ne $sealedRecoveryCapabilityCounts[$capability]) {
        throw 'TM-STATE-RECOVERY-AUTHORITY: exact sealed recovery capability use count drifted'
    }
}
$startupCapabilityCounts = [ordered]@{
    inspect_startup_archive = 4
    StartupArchiveStatus = 6
    StartupValidationMode = 3
}
foreach ($capability in $startupCapabilityCounts.Keys) {
    $actual = @([regex]::Matches($productionText, "\b$capability\b")).Count
    if ($actual -ne $startupCapabilityCounts[$capability]) {
        throw 'TM-STATE-BOOTSTRAP-AUTHORITY: exact read-only startup capability use count drifted'
    }
}
$validatedDirectoryUses = @(
    [regex]::Matches($productionText, '\bValidatedLocalDirectory\b')
)
$settingsConstructorPattern = '(?s)pub\s+fn\s+new\s*\(\s*directory\s*:\s*&ValidatedLocalDirectory\s*\)\s*->\s*Result\s*<\s*Self\s*,\s*StateError\s*>'
$settingsConstructors = @([regex]::Matches($settingsStoreText, $settingsConstructorPattern))
$journalConstructorPattern = '(?s)pub\s+fn\s+new\s*\(\s*directory\s*:\s*&ValidatedLocalDirectory\s*\)\s*->\s*Result\s*<\s*Self\s*,\s*StateError\s*>'
$journalConstructors = @([regex]::Matches($recoveryJournalText, $journalConstructorPattern))
$runStateConstructorPattern = '(?s)pub\s+fn\s+new\s*\(\s*directory\s*:\s*&ValidatedLocalDirectory\s*\)\s*->\s*Result\s*<\s*Self\s*,\s*StateError\s*>'
$runStateConstructors = @([regex]::Matches($runStateText, $runStateConstructorPattern))
$approvedIoMembers = @('Error', 'ErrorKind', 'Result', 'sink')
$ioMemberUses = @([regex]::Matches($productionText, '\bio::(?<member>[A-Za-z_][A-Za-z0-9_]*)'))
$unapprovedIoMembers = @(
    $ioMemberUses |
        Where-Object { $_.Groups['member'].Value -notin $approvedIoMembers }
)
if ($unapprovedIoMembers.Count -ne 0) {
    throw 'TM-STATE-APPROVED-IO: std::io use exceeds bounded writer error/result authority'
}
$presentationDensityEnum = [regex]::Match($settingsValueText, '(?s)pub\s+enum\s+PresentationDensity\s*\{(?<body>.*?)\}').Groups['body'].Value
$presentationSettingsBody = [regex]::Match($settingsValueText, '(?s)pub\s+struct\s+PresentationSettings\s*\{(?<body>.*?)\}').Groups['body'].Value
$strictPortableDispatch = @([regex]::Matches($settingsMigrationText, '(?s)match\s+probe\.schema_version\s*\{\s*1\s*=>\s*decode_portable_v1\(bytes\),\s*SETTINGS_SCHEMA_VERSION\s*=>\s*decode_portable_v2\(bytes\),\s*_\s*=>\s*Err\(StateError::unsupported_version\(\)\),\s*\}')).Count
$strictRecordDispatch = @([regex]::Matches($settingsMigrationText, '(?s)match\s+probe\.schema_version\s*\{\s*1\s*=>\s*decode_settings_v1\(bytes\),\s*SETTINGS_SCHEMA_VERSION\s*=>\s*decode_settings_v2\(bytes\),\s*_\s*=>\s*Err\(RecordValueError::UnsupportedVersion\),\s*\}')).Count
if (($presentationDensityEnum -replace '\s+', '') -ne 'Comfortable,Compact,UltraCompact,' -or
    ($presentationSettingsBody -replace '\s+', '') -ne 'density:PresentationDensity,' -or
    @([regex]::Matches($settingsValueText, 'Self::Comfortable\s*=>\s*"comfortable"')).Count -ne 1 -or
    @([regex]::Matches($settingsValueText, 'Self::Compact\s*=>\s*"compact"')).Count -ne 1 -or
    @([regex]::Matches($settingsValueText, 'Self::UltraCompact\s*=>\s*"ultra_compact"')).Count -ne 1 -or
    $strictPortableDispatch -ne 1 -or $strictRecordDispatch -ne 1 -or
    @([regex]::Matches($settingsMigrationText, 'PresentationSettings::comfortable\(\)')).Count -ne 1 -or
    $settingsMigrationText -match '\.save\s*\(') {
    throw 'TM-STATE-PRESENTATION-CONTRACT: v2 density must remain exact, strict, and migrated only in memory'
}
$exactChildUses = @([regex]::Matches($productionText, '\bexact_child\b'))
$approvedExactChildPattern = 'DurableFileTarget\s*::\s*exact_child\s*\(\s*directory\s*,\s*"(?<child>settings-a\.tms|settings-b\.tms|run-a\.tms|run-b\.tms|recovery-a\.tms|recovery-b\.tms)"\s*\)'
$approvedExactChildUses = @([regex]::Matches($productionText, $approvedExactChildPattern))
$expectedRecordChildren = @(
    'settings-a.tms', 'settings-b.tms', 'run-a.tms', 'run-b.tms',
    'recovery-a.tms', 'recovery-b.tms'
)
$approvedRecordChildren = @(
    $approvedExactChildUses |
        ForEach-Object { $_.Groups['child'].Value } |
        Sort-Object -Unique
)
if ($exactChildUses.Count -ne 6 -or
    $approvedExactChildUses.Count -ne 6 -or
    $approvedRecordChildren.Count -ne 6 -or
    @($expectedRecordChildren | Where-Object { $_ -notin $approvedRecordChildren }).Count -ne 0) {
    throw 'TM-STATE-EXACT-CHILD: state may construct only the six fixed record slots'
}
$authorityText = [regex]::Replace($productionText, $approvedStdIoPattern, '')
$authorityText = [regex]::Replace($authorityText, $approvedPackageReaderIoPattern, '')
$authorityText = [regex]::Replace($authorityText, $approvedPackageWriterIoPattern, '')
$authorityText = [regex]::Replace($authorityText, $approvedPlatformPattern, '')
$authorityText = [regex]::Replace($authorityText, $approvedPackageCapabilityExportPattern, '')
$authorityText = [regex]::Replace($authorityText, $approvedPackageCapabilityImportPattern, '')
$authorityText = [regex]::Replace($authorityText, $approvedSettingsPlatformPattern, '')
$authorityText = [regex]::Replace($authorityText, $approvedCatalogPlatformPattern, '')
$authorityText = [regex]::Replace($authorityText, $approvedRetentionPlatformPattern, '')
$authorityText = [regex]::Replace($authorityText, $approvedRecoveryJournalPlatformPattern, '')
$authorityText = [regex]::Replace($authorityText, $approvedRecoveryPlatformPattern, '')
$authorityText = [regex]::Replace($authorityText, $approvedBootstrapPlatformPattern, '')
$authorityText = [regex]::Replace($authorityText, $approvedStoreCandidatePattern, '')
$authorityText = [regex]::Replace($authorityText, $approvedRecoveryStorePattern, '')
$authorityText = [regex]::Replace($authorityText, $approvedBootstrapStorePattern, '')
$authorityText = [regex]::Replace($authorityText, $approvedMaintenanceStoreControlPattern, '')
$authorityText = [regex]::Replace($authorityText, $approvedMaintenanceCoordinatorStdPattern, '')
$authorityText = [regex]::Replace($authorityText, $approvedMaintenanceSchedulerStdPattern, '')
$authorityText = [regex]::Replace($authorityText, $approvedMaintenanceWorkerStdPattern, '')
$authorityText = [regex]::Replace($authorityText, $approvedMaintenanceOwnerStdPattern, '')

$publicPathPattern = '(?s)\bpub(?:\([^)]*\))?\s+(?:(?:const|async|unsafe)\s+)*fn\s+\w+[^;{]*(?:std::path::)?(?:Path|PathBuf)\b[^;{]*[;{]'
if ($productionText -match $publicPathPattern) {
    throw 'TM-STATE-ARBITRARY-PATH: public state API must not accept filesystem paths'
}
$publicStreamAuthorityPattern = '(?s)\bpub(?:\([^)]*\))?\s+(?:(?:const|async|unsafe)\s+)*fn\s+\w+(?=[^;{]*\b(?:Read|Write)\b)[^;{]*[;{]'
if ($productionText -match $publicStreamAuthorityPattern) {
    throw 'TM-STATE-STREAM-AUTHORITY: public state API must use controlled file capabilities, not generic streams'
}
$publicRecordAuthorityPattern = '(?m)^\s*pub\s+(?:use\s+record\b|mod\s+record\b|struct\s+RedundantRecordStore\b)'
if ($productionText -match $publicRecordAuthorityPattern) {
    throw 'TM-STATE-RECORD-VISIBILITY: generic record filesystem authority must remain crate-private'
}
$approvedBackupStageWriterPattern = '(?s)\bpub\s+fn\s+write_to_backup_stage\s*\(.*?\bdestination\s*:\s*&mut\s+BackupStagedFile\s*,?\s*\)\s*->\s*Result\s*<\s*PackageReceipt\s*,\s*StateError\s*>\s*\{'
$approvedBackupStageWriters = @(
    [regex]::Matches($productionText, $approvedBackupStageWriterPattern)
)
if ($approvedBackupStageWriters.Count -ne 1) {
    throw 'TM-STATE-BACKUP-DIRECTORY-AUTHORITY: exactly one typed backup-stage writer is allowed'
}
$approvedBackupStageVerifierPattern = '(?s)\bpub\s+fn\s+verify_backup_stage\s*\(\s*source\s*:\s*&BackupStagedFile\s*,?\s*\)\s*->\s*Result\s*<\s*VerifiedBackupPackage\s*,\s*StateError\s*>\s*\{'
$approvedBackupStageVerifiers = @(
    [regex]::Matches($productionText, $approvedBackupStageVerifierPattern)
)
if ($approvedBackupStageVerifiers.Count -ne 1) {
    throw 'TM-STATE-BACKUP-DIRECTORY-AUTHORITY: exactly one typed backup-stage verifier is allowed'
}
$approvedVerifiedStageCopyPattern = '(?s)\bpub\s+fn\s+copy_verified_stage_to_durable\s*\(\s*source\s*:\s*&BackupStagedFile\s*,\s*verified\s*:\s*&VerifiedBackupPackage\s*,\s*destination\s*:\s*&mut\s+DurableStagedFile\s*,?\s*\)\s*->\s*Result\s*<\s*PackageReceipt\s*,\s*StateError\s*>\s*\{'
$approvedVerifiedStageCopies = @(
    [regex]::Matches($productionText, $approvedVerifiedStageCopyPattern)
)
if ($approvedVerifiedStageCopies.Count -ne 1) {
    throw 'TM-STATE-BACKUP-DIRECTORY-AUTHORITY: exactly one verified stage-to-durable copy is allowed'
}
$approvedVerifiedCandidateStageWriterPattern = '(?s)\bpub\s+fn\s+write_verified_candidate_to_backup_stage\s*\(\s*settings\s*:\s*&PortableSettingsCandidate\s*,\s*mut\s+database\s*:\s*VerifiedBackupCandidateReader\s*<\s*''_\s*>\s*,\s*compression\s*:\s*BackupCompression\s*,\s*metadata\s*:\s*BackupMetadata\s*,\s*destination\s*:\s*&mut\s+BackupStagedFile\s*,?\s*\)\s*->\s*Result\s*<\s*PackageReceipt\s*,\s*StateError\s*>\s*\{'
$approvedVerifiedCandidateStageWriters = @(
    [regex]::Matches($productionText, $approvedVerifiedCandidateStageWriterPattern)
)
if ($approvedVerifiedCandidateStageWriters.Count -ne 1) {
    throw 'TM-STATE-BACKUP-DIRECTORY-AUTHORITY: exactly one verified-candidate stage writer is allowed'
}
$approvedMaintenanceBackupControlPattern = '(?s)\bpub\s+fn\s+backup_control\s*\(\s*&self\s*\)\s*->\s*Result\s*<\s*BackupControl\s*,\s*StateError\s*>\s*\{'
$approvedMaintenanceBackupControls = @(
    [regex]::Matches($productionText, $approvedMaintenanceBackupControlPattern)
)
if ($approvedMaintenanceBackupControls.Count -ne 1) {
    throw 'TM-STATE-STORE-AUTHORITY: exactly one permit-linked backup control is allowed'
}
$backupAuthorityText = [regex]::Replace(
    $productionText,
    $approvedBackupStageWriterPattern,
    'pub fn write_to_backup_stage() {'
)
$backupAuthorityText = [regex]::Replace(
    $backupAuthorityText,
    $approvedBackupStageVerifierPattern,
    'pub fn verify_backup_stage() {'
)
$backupAuthorityText = [regex]::Replace(
    $backupAuthorityText,
    $approvedVerifiedStageCopyPattern,
    'pub fn copy_verified_stage_to_durable() {'
)
$backupAuthorityText = [regex]::Replace(
    $backupAuthorityText,
    $approvedVerifiedCandidateStageWriterPattern,
    'pub fn write_verified_candidate_to_backup_stage() {'
)
$backupAuthorityText = [regex]::Replace(
    $backupAuthorityText,
    $approvedMaintenanceBackupControlPattern,
    'pub fn backup_control() {'
)
$publicBackupDirectoryAuthorityPattern = '(?s)\bpub\s+(?:(?:const|async|unsafe)\s+)*fn\s+\w+[^;{]*\b(?:BackupDirectoryEntry|BackupDirectoryGeneration|BackupStagedFile)\b[^;{]*[;{]'
if ($backupAuthorityText -match $publicBackupDirectoryAuthorityPattern) {
    throw 'TM-STATE-BACKUP-DIRECTORY-AUTHORITY: raw platform backup tokens must remain inside typed catalog and retention operations'
}
$publicStoreAuthorityPattern = '(?s)\bpub\s+(?:(?:const|async|unsafe)\s+)*fn\s+\w+[^;{]*\bVerifiedBackupCandidateReader\b[^;{]*[;{]'
if ($backupAuthorityText -match $publicStoreAuthorityPattern) {
    throw 'TM-STATE-STORE-AUTHORITY: raw store capabilities must remain inside the exact maintenance and package bridges'
}
$forbiddenAuthorityPattern = '(?s)https?://|\bstd\b|\btokenmaster_platform\b|\btokenmaster_store\b|\bmacro_rules\s*!|\b(?:Command|TcpStream|TcpListener|UdpSocket)\b|\b(?:slint|rusqlite|tokio|reqwest|ureq|webbrowser|headless_chrome|zip|tar)::|\b(?:SELECT|INSERT|UPDATE|DELETE\s+FROM|PRAGMA)\b|\b(?:include|include_str|include_bytes)!\s*\(|#\s*\[\s*path\s*=|powershell(?:\.exe)?|cmd(?:\.exe)?|bash(?:\.exe)?|\bsh\s+-c\b|\bAuthorization\b|\bBearer\s'
if ($authorityText -cmatch $forbiddenAuthorityPattern) {
    throw 'TM-STATE-FORBIDDEN-AUTHORITY: state source contains standard-library/platform/macro/filesystem/network/shell/process/SQL/UI/archive/external-source authority'
}
if ($validatedDirectoryUses.Count -ne 17 -or
    $settingsConstructors.Count -ne 1 -or
    $journalConstructors.Count -ne 1 -or
    $runStateConstructors.Count -ne 1 -or
    $productionText -cmatch '\.\s*as_path\s*\(') {
    throw 'TM-STATE-VALIDATED-DIRECTORY: directory capability is limited to fixed record, settings, run, journal, recovery, and bootstrap bindings'
}

if ($SourceOnly) {
    [ordered]@{
        result = 'pass'
        scope = 'source-only'
        package = 'tokenmaster-state'
        workspace_member_count = $stateWorkspaceMembers.Count
        binary_target_count = 0
        direct_production_dependency_count = $directProductionDependencies.Count
        rust_source_file_count = $rustFiles.Count
        approved_std_io_import_count = $approvedStdIoImports.Count + $approvedPackageReaderIoImports.Count + $approvedPackageWriterIoImports.Count
        approved_maintenance_std_import_count = $approvedMaintenanceCoordinatorStdImports.Count + $approvedMaintenanceSchedulerStdImports.Count + $approvedMaintenanceWorkerStdImports.Count + $approvedMaintenanceOwnerStdImports.Count
        approved_store_candidate_import_count = $approvedStoreCandidateImports.Count
        approved_recovery_store_import_count = $approvedRecoveryStoreImports.Count
        approved_bootstrap_store_import_count = $approvedBootstrapStoreImports.Count
        approved_maintenance_store_control_import_count = $approvedMaintenanceStoreControlImports.Count
        approved_platform_import_count = $approvedPlatformImports.Count + $approvedPackageCapabilityExports.Count + $approvedPackageCapabilityImports.Count + $approvedSettingsPlatformImports.Count + $approvedRunStatePlatformImports.Count + $approvedCatalogPlatformImports.Count + $approvedRetentionPlatformImports.Count + $approvedRecoveryJournalPlatformImports.Count + $approvedRecoveryPlatformImports.Count + $approvedBootstrapPlatformImports.Count
        validated_directory_capability_use_count = $validatedDirectoryUses.Count
        forbidden_authority_count = 0
        arbitrary_path_constructor_count = 0
        fault_matrix_anchor_count = $faultMatrixAnchors.Count
    } | ConvertTo-Json -Compress
    return
}

$cargo = (Get-Command cargo.exe -CommandType Application -ErrorAction Stop).Source
$metadataJson = & $cargo +1.97.0 metadata --locked --format-version 1 --manifest-path $rootManifest
if ($LASTEXITCODE -ne 0) {
    throw 'TM-STATE-METADATA: cargo metadata failed'
}
$metadata = $metadataJson | ConvertFrom-Json -Depth 100
$statePackages = @($metadata.packages | Where-Object { $_.name -eq 'tokenmaster-state' })
if ($statePackages.Count -ne 1) {
    throw 'TM-STATE-PACKAGE: tokenmaster-state must resolve exactly once'
}
$metadataStateMembers = @(
    $metadata.workspace_members |
        Where-Object { [string]$_ -eq [string]$statePackages[0].id }
)
if ($metadataStateMembers.Count -ne 1) {
    throw 'TM-STATE-WORKSPACE: tokenmaster-state must resolve as one exact workspace member'
}
$metadataDependencies = @(
    $statePackages[0].dependencies |
        Where-Object { $null -eq $_.kind } |
        ForEach-Object { $_.name } |
        Sort-Object -Unique
)
if ($metadataDependencies.Count -ne $expectedDependencies.Count -or
    @($expectedDependencies | Where-Object { $_ -notin $metadataDependencies }).Count -ne 0) {
    throw "TM-STATE-DEPENDENCIES: metadata dependency set drifted: $($metadataDependencies -join ', ')"
}
$zstdDependencies = @(
    $statePackages[0].dependencies |
        Where-Object { $_.name -eq 'zstd' -and $null -eq $_.kind }
)
if ($zstdDependencies.Count -ne 1 -or
    $zstdDependencies[0].req -ne '=0.13.3' -or
    $zstdDependencies[0].uses_default_features -ne $false -or
    @($zstdDependencies[0].features).Count -ne 0) {
    throw 'TM-STATE-ZSTD-PIN: resolved zstd dependency contract drifted'
}
$ageDependencies = @(
    $statePackages[0].dependencies |
        Where-Object { $_.name -eq 'age' -and $null -eq $_.kind }
)
if ($ageDependencies.Count -ne 1 -or
    $ageDependencies[0].req -ne '=0.12.1' -or
    $ageDependencies[0].uses_default_features -ne $false -or
    @($ageDependencies[0].features).Count -ne 0) {
    throw 'TM-STATE-AGE-PIN: resolved age dependency contract drifted'
}
$binaryTargets = @($statePackages[0].targets | Where-Object { $_.kind -contains 'bin' })
if ($binaryTargets.Count -ne 0) {
    throw 'TM-STATE-BINARY-TARGET: metadata contains a state binary target'
}

$treeText = (& $cargo +1.97.0 tree -p tokenmaster-state -e normal --prefix none --format '{p}' --manifest-path $rootManifest) -join "`n"
if ($LASTEXITCODE -ne 0) {
    throw 'TM-STATE-TREE: cargo dependency tree failed'
}
if ($treeText -match '(?m)^(?:zip|tar|tokio|reqwest|ureq|slint|webbrowser|headless_chrome)\s+v') {
    throw 'TM-STATE-TRANSITIVE-AUTHORITY: forbidden dependency entered the state tree'
}
$featureTreeText = (& $cargo +1.97.0 tree -p tokenmaster-state -e features --manifest-path $rootManifest) -join "`n"
if ($LASTEXITCODE -ne 0) {
    throw 'TM-STATE-TREE: cargo feature tree failed'
}
if ($featureTreeText -match '(?i)zstd(?:-safe|-sys)? feature "(?:zstdmt|training|legacy|experimental)"') {
    throw 'TM-STATE-ZSTD-FEATURES: forbidden zstd feature entered the state tree'
}
if ($featureTreeText -match '(?i)\bage feature "(?:armor|async|cli-common|plugin|ssh|unstable|web-sys)"') {
    throw 'TM-STATE-AGE-FEATURES: forbidden age feature entered the state tree'
}

[ordered]@{
    result = 'pass'
    scope = 'workspace'
    package = 'tokenmaster-state'
    workspace_member_count = $metadataStateMembers.Count
    binary_target_count = $binaryTargets.Count
    direct_production_dependencies = $metadataDependencies
    direct_production_dependency_count = $metadataDependencies.Count
    rust_source_file_count = $rustFiles.Count
    approved_std_io_import_count = $approvedStdIoImports.Count + $approvedPackageReaderIoImports.Count + $approvedPackageWriterIoImports.Count
    approved_maintenance_std_import_count = $approvedMaintenanceCoordinatorStdImports.Count + $approvedMaintenanceSchedulerStdImports.Count + $approvedMaintenanceWorkerStdImports.Count + $approvedMaintenanceOwnerStdImports.Count
    approved_store_candidate_import_count = $approvedStoreCandidateImports.Count
    approved_recovery_store_import_count = $approvedRecoveryStoreImports.Count
    approved_bootstrap_store_import_count = $approvedBootstrapStoreImports.Count
    approved_maintenance_store_control_import_count = $approvedMaintenanceStoreControlImports.Count
    approved_platform_import_count = $approvedPlatformImports.Count + $approvedPackageCapabilityExports.Count + $approvedPackageCapabilityImports.Count + $approvedSettingsPlatformImports.Count + $approvedRunStatePlatformImports.Count + $approvedCatalogPlatformImports.Count + $approvedRetentionPlatformImports.Count + $approvedRecoveryJournalPlatformImports.Count + $approvedRecoveryPlatformImports.Count + $approvedBootstrapPlatformImports.Count
    validated_directory_capability_use_count = $validatedDirectoryUses.Count
    forbidden_authority_count = 0
    arbitrary_path_constructor_count = 0
    forbidden_transitive_dependency_count = 0
    fault_matrix_anchor_count = $faultMatrixAnchors.Count
} | ConvertTo-Json -Compress
