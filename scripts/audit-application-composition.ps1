[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$RepositoryRoot,
    [switch]$SourceOnly
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$rootManifest = Join-Path $root 'Cargo.toml'
$appRoot = Join-Path $root 'crates\app'
$appManifest = Join-Path $appRoot 'Cargo.toml'
$appSource = Join-Path $appRoot 'src'
$recoveryAdversarial = Join-Path $appRoot 'tests\recovery_adversarial_contract.rs'
$desktopManifest = Join-Path $root 'crates\desktop\Cargo.toml'
$reminderRuntimePath = Join-Path $root 'crates\runtime\src\reminder\runtime.rs'

foreach ($required in @(
    $rootManifest,
    $appManifest,
    $appSource,
    $recoveryAdversarial,
    $desktopManifest,
    $reminderRuntimePath
)) {
    if (-not (Test-Path -LiteralPath $required)) {
        throw "TM-APP-MISSING-BOUNDARY: $([System.IO.Path]::GetFileName($required))"
    }
}
$recoveryAdversarialText = [System.IO.File]::ReadAllText($recoveryAdversarial)
$recoveryAdversarialAnchors = @(
    'fn application_recovery_and_migration_matrix_remains_executable()',
    'fn application_gate_is_bound_to_the_complete_state_recovery_matrix()',
    'mod automatic_recovery_contract;',
    'mod maintenance_contract;',
    'mod recovery_journal_contract;',
    'mod restore_contract;'
)
foreach ($anchor in $recoveryAdversarialAnchors) {
    if ([regex]::Matches($recoveryAdversarialText, [regex]::Escape($anchor)).Count -ne 1) {
        throw "TM-APP-RECOVERY-ADVERSARIAL: missing exact anchor $anchor"
    }
}

$manifestText = [System.IO.File]::ReadAllText($appManifest)
$desktopManifestText = [System.IO.File]::ReadAllText($desktopManifest)
if ([regex]::Matches($manifestText, '\[\[bin\]\]').Count -ne 1 -or
    $manifestText -notmatch 'name\s*=\s*"TokenMaster"' -or
    $manifestText -notmatch 'path\s*=\s*"src/main\.rs"') {
    throw 'TM-APP-BINARY-OWNER: tokenmaster-app must declare the sole TokenMaster binary'
}
if ($desktopManifestText -match '\[\[bin\]\]|name\s*=\s*"TokenMaster"') {
    throw 'TM-APP-DUPLICATE-BINARY: tokenmaster-desktop must remain library-only'
}
if ($manifestText -match '\btokenmaster-m0\b|[\\/]probe-app\b|\brenderer-femtovg\b') {
    throw 'TM-APP-PROBE-DEPENDENCY: production composition must not depend on the M0 probe'
}

$rustFiles = @(
    Get-ChildItem -LiteralPath $appSource -Recurse -File -Filter '*.rs' |
        Where-Object { $_.Name -notlike '*_tests.rs' }
)
if ($rustFiles.Count -ne 8) {
    throw 'TM-APP-FILE-COUNT: application composition must contain exactly eight Rust files'
}
$productionText = ($rustFiles | ForEach-Object {
    [System.IO.File]::ReadAllText($_.FullName)
}) -join "`n"
$applicationText = [System.IO.File]::ReadAllText((Join-Path $appSource 'application.rs'))
$dataRootText = [System.IO.File]::ReadAllText((Join-Path $appSource 'data_root.rs'))
$notificationText = [System.IO.File]::ReadAllText((Join-Path $appSource 'notification.rs'))
$operationText = [System.IO.File]::ReadAllText((Join-Path $appSource 'operation.rs'))
$commandText = [System.IO.File]::ReadAllText((Join-Path $appSource 'command.rs'))
$stateText = [System.IO.File]::ReadAllText((Join-Path $appSource 'state.rs'))
$reminderRuntimeText = [System.IO.File]::ReadAllText($reminderRuntimePath)

if ($productionText -match 'LiveRuntime::start_notified\(') {
    throw 'TM-APP-UNGUARDED-LIVE: live runtime must consume the startup lease guard'
}

foreach ($contract in @(
    @{ Name = 'TM-APP-STATE-OWNER'; Pattern = 'ApplicationStateOwner::open\('; Count = 1 },
    @{ Name = 'TM-APP-PREFLIGHT'; Pattern = '\.prepare\(&data_root\)'; Count = 1 },
    @{ Name = 'TM-APP-LIVE-OWNER'; Pattern = 'LiveRuntime::start_notified_guarded\('; Count = 1 },
    @{ Name = 'TM-APP-MAINTENANCE-OWNER'; Pattern = 'BackupMaintenanceRuntime::spawn\('; Count = 1 },
    @{ Name = 'TM-APP-COMMAND-COORDINATOR'; Pattern = 'ApplicationCommandCoordinator::new\('; Count = 1 },
    @{ Name = 'TM-APP-OPERATION-WORKER'; Pattern = 'ApplicationOperationWorker::spawn_with_payload\('; Count = 1 },
    @{ Name = 'TM-APP-OPERATION-THREAD'; Pattern = '"tokenmaster-operation-worker"'; Count = 1 },
    @{ Name = 'TM-APP-OPERATION-WAKE'; Pattern = 'sync_channel\(1\)'; Count = 1 },
    @{ Name = 'TM-APP-OPERATION-ACTUAL-START'; Pattern = 'ApplicationOperationWorker::spawn_with_payload\(move \|permit, payload\| \{\s*let _ = command_notifier\s*\.publish_operation\(Some\(application_operation_running\(permit\.command\(\)\)\)\)'; Count = 1 },
    @{ Name = 'TM-APP-BACKUP-COMMAND'; Pattern = 'ApplicationCommand::Backup,\s*ApplicationOperationPayload::Empty\)\s*=>\s*\{\s*execute_manual_backup_command\('; Count = 1 },
    @{ Name = 'TM-APP-OPERATION-JOIN'; Pattern = 'self\.commands\.shutdown\(\)'; Count = 1 },
    @{ Name = 'TM-APP-CONFIG-SEALED-TARGET'; Pattern = 'pub\(crate\)\s+fn export_config\([\s\S]{0,256}?mut target:\s*SelectedOutputFile'; Count = 1 },
    @{ Name = 'TM-APP-CONFIG-SEALED-SOURCE'; Pattern = 'pub\(crate\)\s+fn preview_config_import\([\s\S]{0,256}?source:\s*SelectedInputFile'; Count = 1 },
    @{ Name = 'TM-APP-CONFIG-BOUNDED-STAGE'; Pattern = '\.create_staged\(MAX_CONFIG_PACKAGE_BYTES\)'; Count = 1 },
    @{ Name = 'TM-APP-CONFIG-BOUNDED-READ'; Pattern = '\.open_reader\(MAX_CONFIG_PACKAGE_BYTES\)'; Count = 1 },
    @{ Name = 'TM-APP-COMPACT-EXPORT-REQUEST'; Pattern = 'ApplicationOperationRequest::compact_backup\(output\)'; Count = 1 },
    @{ Name = 'TM-APP-COMPACT-EXPORT-VERIFIED-COPY'; Pattern = 'BackupPackage::copy_verified_stage_to_durable\('; Count = 1 },
    @{ Name = 'TM-APP-ENCRYPTED-EXPORT-REQUEST'; Pattern = 'ApplicationOperationRequest::encrypted_backup\(output,\s*passphrase\)'; Count = 1 },
    @{ Name = 'TM-APP-ENCRYPTED-EXPORT-PASSPHRASE'; Pattern = 'BackupPassphrase::existing\(&mut secret\)'; Count = 1 },
    @{ Name = 'TM-APP-ENCRYPTED-EXPORT-WRITE'; Pattern = 'EncryptedBackupPackage::encrypt\('; Count = 1 },
    @{ Name = 'TM-APP-BACKUP-POLICY-REQUEST'; Pattern = 'ApplicationOperationRequest::update_backup_policy\('; Count = 1 },
    @{ Name = 'TM-APP-BACKUP-POLICY-COMMIT'; Pattern = 'state\s*\.update_backup_policy\('; Count = 1 },
    @{ Name = 'TM-APP-RESTART-PAUSE'; Pattern = 'self\.commands\s*\.pause_admission\(\)'; Count = 2 },
    @{ Name = 'TM-APP-RESTART-RESUME'; Pattern = 'self\.commands\s*\.resume_admission\(\)'; Count = 2 },
    @{ Name = 'TM-APP-RESTART-GUARD'; Pattern = '\.acquire_runtime_guard\(&self\.data_root\)'; Count = 2 },
    @{ Name = 'TM-APP-RESTORE-BINDING'; Pattern = '\.bind_backup_selection\(selection\)'; Count = 2 },
    @{ Name = 'TM-APP-RESTORE-CURRENT-BIND'; Pattern = '\.bind_current_selection\(&self\.backups'; Count = 1 },
    @{ Name = 'TM-APP-RESTORE-DYNAMIC-PIN'; Pattern = 'retention\.delete_next_protected\('; Count = 1 },
    @{ Name = 'TM-APP-RESTORE-PIN-DROP'; Pattern = 'impl Drop for ApplicationBackupSelectionPin'; Count = 1 },
    @{ Name = 'TM-APP-RESTORE-PROTECTED'; Pattern = '\.start_protected_maintenance\('; Count = 2 },
    @{ Name = 'TM-APP-PRE-RESTORE'; Pattern = 'wait_for_mandatory_backup\(\s*&maintenance,\s*MaintenancePurpose::PreRestore\s*\)'; Count = 2 },
    @{ Name = 'TM-APP-RESTORE-SAFETY'; Pattern = 'RestoreSafety::PreRestoreBackupPublished\('; Count = 2 },
    @{ Name = 'TM-APP-SELECTED-RESTORE'; Pattern = '(?:self\.)?state\.restore_selected\('; Count = 2 },
    @{ Name = 'TM-APP-RECOVERY-LAUNCH'; Pattern = '\.bind_recovery_launch\(receipt\)'; Count = 3 },
    @{ Name = 'TM-APP-REBUILD-BINDING'; Pattern = 'ApplicationCommand::Rebuild,\s*ApplicationOperationPayload::Empty\)\s*=>\s*\{\s*execute_rebuild_operation\('; Count = 1 },
    @{ Name = 'TM-APP-REBUILD-AUTHORITATIVE'; Pattern = 'state\.reconstruct_definitively_corrupt\('; Count = 1 },
    @{ Name = 'TM-APP-REBUILD-RECONCILE'; Pattern = '\.refresh_now\(RefreshUrgency::Recovery\)'; Count = 1 },
    @{ Name = 'TM-APP-REBUILD-RECONCILE-WAIT'; Pattern = 'wait_for_reconstructed_reconciliation\(&started\.live\)'; Count = 1 },
    @{ Name = 'TM-APP-REBUILD-DURABLE-RECONCILE'; Pattern = 'RecoveryLaunchDecision::Start \{ \.\. \} \| RecoveryLaunchDecision::SafeMode \{ \.\. \}[\s\S]{0,256}?RecoveryPhase::Complete && journal\.backup\(\)\.is_none\(\)'; Count = 1 },
    @{ Name = 'TM-APP-REBUILD-COLD-RECONCILE'; Pattern = 'if preflight\.requires_source_reconciliation\(\) \{[\s\S]{0,512}?ApplicationStartBoundary::BeforeReconstructionReconciliation[\s\S]{0,512}?start_reconstructed_bundle\('; Count = 1 },
    @{ Name = 'TM-APP-REBUILD-RETRY-RECONCILE'; Pattern = 'if source_reconciliation_required \{[\s\S]{0,256}?permit\.begin_irreversible\(\)[\s\S]{0,512}?start_reconstructed_bundle\('; Count = 1 },
    @{ Name = 'TM-APP-REBUILD-ATOMIC'; Pattern = 'RecoveryBoundary::BeforeJournalPublication[\s\S]{0,256}?on_irreversible\(\)'; Count = 1 },
    @{ Name = 'TM-APP-MANUAL-BACKUP-ATOMIC'; Pattern = 'fn execute_manual_backup_command\([\s\S]{0,1024}?permit\.begin_irreversible\(\)[\s\S]{0,512}?publish_atomic_operation\(reliable_state, permit\.command\(\)\)'; Count = 1 },
    @{ Name = 'TM-APP-ATOMIC-PROJECTION'; Pattern = 'publish_atomic_operation\(reliable_state, permit\.command\(\)\)'; Count = 8 },
    @{ Name = 'TM-APP-RESTORED-MIGRATION'; Pattern = 'fn start_restored_bundle\('; Count = 1 },
    @{ Name = 'TM-APP-PRE-MIGRATION'; Pattern = 'wait_for_mandatory_backup\([\s\S]{0,96}?MaintenancePurpose::PreMigration\s*\)'; Count = 2 },
    @{ Name = 'TM-APP-POST-MIGRATION'; Pattern = 'wait_for_mandatory_backup\([\s\S]{0,96}?MaintenancePurpose::PostMigration\s*\)'; Count = 2 },
    @{ Name = 'TM-APP-MIGRATION-PENDING'; Pattern = '\.require_post_migration\('; Count = 2 },
    @{ Name = 'TM-APP-MIGRATION-COMPLETE'; Pattern = '\.complete_post_migration\('; Count = 2 },
    @{ Name = 'TM-APP-ATOMIC-MAINTENANCE-WAIT'; Pattern = '\.submit_and_wait\('; Count = 2 },
    @{ Name = 'TM-APP-CLEAN-STATE'; Pattern = '\.mark_clean\(\)'; Count = 1 },
    @{ Name = 'TM-APP-QUOTA-OWNER'; Pattern = 'CodexQuotaRuntime::start_notified\('; Count = 1 },
    @{ Name = 'TM-APP-REMINDER-OWNER'; Pattern = 'BenefitReminderRuntime::start_notified\('; Count = 1 },
    @{ Name = 'TM-APP-NOTIFICATION-COORDINATOR'; Pattern = 'ReminderPresentationCoordinator::start\('; Count = 1 },
    @{ Name = 'TM-APP-NOTIFICATION-PORT'; Pattern = 'RuntimeReminderPresentationPort::new\('; Count = 1 },
    @{ Name = 'TM-APP-NOTIFICATION-PUMP'; Pattern = 'presentation\.pump\(\)'; Count = 1 },
    @{ Name = 'TM-APP-CONTROLLER'; Pattern = 'DesktopController::open\('; Count = 1 },
    @{ Name = 'TM-APP-SESSION-DETAIL-ROUTER'; Pattern = 'DesktopSessionDetailIntentRouter::new\('; Count = 1 },
    @{ Name = 'TM-APP-SESSION-DETAIL-SHELL'; Pattern = 'DesktopShell::new_with_reliable_state_and_session_sink\('; Count = 1 },
    @{ Name = 'TM-APP-SESSION-DETAIL-INSTALL'; Pattern = 'session_detail_router\s*\.install\(Rc::new\(ApplicationSessionDetailIntentSink::new\('; Count = 1 },
    @{ Name = 'TM-APP-SESSION-DETAIL-CURRENT-BUNDLE'; Pattern = 'bundle\.controller\.request_session_detail\(intent\)'; Count = 1 },
    @{ Name = 'TM-APP-SESSION-DETAIL-SAFE-MODE'; Pattern = 'let Some\(bundle\) = slot\.as_ref\(\) else \{\s*return DesktopSessionDetailIntentAdmission::Rejected'; Count = 1 },
    @{ Name = 'TM-APP-SESSION-DETAIL-NONBLOCKING'; Pattern = 'let Ok\(slot\) = bundle\.try_lock\(\) else \{\s*return DesktopSessionDetailIntentAdmission::Rejected'; Count = 1 },
    @{ Name = 'TM-APP-BRIDGE'; Pattern = '\.snapshot_bridge\('; Count = 1 },
    @{ Name = 'TM-APP-EVENT-LOOP'; Pattern = 'slint::run_event_loop\('; Count = 1 },
    @{ Name = 'TM-APP-PORTABLE-MARKER'; Pattern = '"tokenmaster\.portable"'; Count = 1 },
    @{ Name = 'TM-APP-ARCHIVE-NAME'; Pattern = '"tokenmaster\.sqlite3"'; Count = 1 }
)) {
    $actual = [regex]::Matches($productionText, $contract.Pattern).Count
    if ($actual -ne $contract.Count) {
        throw "$($contract.Name): expected $($contract.Count), observed $actual"
    }
}

$orderedRestoreLaunches = [regex]::Matches(
    $applicationText,
    '\.bind_recovery_launch\(receipt\)\?;[\s\S]{0,1024}?start_restored_bundle\('
).Count
if ($orderedRestoreLaunches -ne 2) {
    throw 'TM-APP-RESTORE-RECOVERY-ORDER: recovery receipt must bind before restored lifecycle work'
}

if ($applicationText -notmatch 'Weak<Mutex<ApplicationBundleSlot>>' -or
    $applicationText -notmatch 'impl WorkerCompletionNotifier for ApplicationRuntimeNotifier') {
    throw 'TM-APP-WEAK-NOTIFIER: runtime completion notifier must retain only weak application state'
}
if ([regex]::Matches($applicationText, 'slot\.generation != self\.bundle_generation').Count -ne 1) {
    throw 'TM-APP-BUNDLE-GENERATION: obsolete runtime notifiers must fail closed'
}
if ($applicationText -match '\b(slint::Timer|std::thread|thread::spawn|thread::sleep)\b') {
    throw 'TM-APP-POLLING: application composition must not add a timer or polling thread'
}
$notificationWorkerCount = [regex]::Matches($notificationText, 'thread::Builder::new\(\)').Count
if ($notificationWorkerCount -ne 1 -or
    [regex]::Matches($notificationText, '"tokenmaster-notification-receipt"').Count -ne 1 -or
    $notificationText -match '\b(?:thread::spawn|thread::sleep|slint::Timer|VecDeque|sync_channel)\b') {
    throw 'TM-APP-NOTIFICATION-WORKER: notifications must own one named condition-variable receipt worker'
}
$operationWorkerBuilderCount = [regex]::Matches(
    $operationText,
    '(?:thread::)?Builder::new\(\)'
).Count
if ($operationWorkerBuilderCount -ne 1) {
    throw "TM-APP-OPERATION-SPAWN: expected 1, observed $operationWorkerBuilderCount"
}

$reminderSealedPayloadCount = [regex]::Matches(
    $commandText,
    'ApplicationCommand::UpdateReminderPolicy,\s*payload:\s*ApplicationOperationPayload::ReminderPolicy\(update\)'
).Count
if ($reminderSealedPayloadCount -ne 1 -or
    [regex]::Matches($commandText, 'pub\(crate\)\s+struct ApplicationReminderPolicyUpdate').Count -ne 1 -or
    $commandText -notmatch 'ApplicationReminderPolicyUpdate\(\[redacted\]\)') {
    throw 'TM-APP-REMINDER-SEALED: one redacted typed reminder payload must remain bound to UpdateReminderPolicy'
}
$reminderProfileFunction = [regex]::Match(
    $stateText,
    '(?s)fn reminder_profile_from_settings\(.*?\r?\n\}\r?\n\r?\nimpl fmt::Debug'
).Value
if ([string]::IsNullOrWhiteSpace($reminderProfileFunction) -or
    [regex]::Matches($reminderProfileFunction, '\.unwrap_or\(0\)\s*\.checked_add\(1\)\s*\.filter\(\|value\| \*value <= i64::MAX as u64\)').Count -ne 1) {
    throw 'TM-APP-REMINDER-GENERATION: settings generation must map exactly to global profile revision N + 1'
}
$reminderUpdateFunction = [regex]::Match(
    $stateText,
    '(?s)pub\(crate\) fn update_reminder_policy\(.*?\r?\n    \}\r?\n\r?\n    pub\(crate\) fn synchronize_reminder_profile'
).Value
$pendingIndex = $reminderUpdateFunction.IndexOf('.swap(REMINDER_SYNC_PENDING, Ordering::AcqRel)', [System.StringComparison]::Ordinal)
$visiblePendingIndex = $reminderUpdateFunction.IndexOf('on_irreversible().is_err()', [System.StringComparison]::Ordinal)
$settingsSaveIndex = $reminderUpdateFunction.IndexOf('.save(&value)', [System.StringComparison]::Ordinal)
if ([string]::IsNullOrWhiteSpace($reminderUpdateFunction) -or $pendingIndex -lt 0 -or
    $visiblePendingIndex -le $pendingIndex -or $settingsSaveIndex -le $visiblePendingIndex) {
    throw 'TM-APP-REMINDER-SETTINGS-FIRST: durable desired settings must follow acknowledged Pending publication before archive synchronization'
}
$reminderSynchronizeFunction = [regex]::Match(
    $stateText,
    '(?s)pub\(crate\) fn synchronize_reminder_profile\(.*?\r?\n    \}\r?\n\r?\n    fn reminder_sync_state'
).Value
$syncPendingIndex = $reminderSynchronizeFunction.IndexOf('store(REMINDER_SYNC_PENDING, Ordering::Release)', [System.StringComparison]::Ordinal)
$syncStoreIndex = $reminderSynchronizeFunction.IndexOf('.set_benefit_reminder_global_profile(&profile)', [System.StringComparison]::Ordinal)
$syncSynchronizedIndex = $reminderSynchronizeFunction.IndexOf('store(REMINDER_SYNC_SYNCHRONIZED, Ordering::Release)', [System.StringComparison]::Ordinal)
if ([string]::IsNullOrWhiteSpace($reminderSynchronizeFunction) -or $syncPendingIndex -lt 0 -or
    $syncStoreIndex -le $syncPendingIndex -or $syncSynchronizedIndex -le $syncStoreIndex) {
    throw 'TM-APP-REMINDER-SYNC-STATE: Pending must precede and Synchronized must follow the global profile commit'
}
$reminderOperationBinding = [regex]::Match(
    $applicationText,
    '(?s)\(\s*ApplicationCommand::UpdateReminderPolicy,\s*ApplicationOperationPayload::ReminderPolicy\(update\),\s*\)\s*=>\s*\{.*?\r?\n\s*\}\r?\n\s*\(ApplicationCommand::RestoreData'
).Value
$reminderStateUpdateIndex = $reminderOperationBinding.IndexOf('state.update_reminder_policy(permit, policy', [System.StringComparison]::Ordinal)
$reminderSynchronizeIndex = $reminderOperationBinding.IndexOf('synchronize_reminder_policy_after_settings(', [System.StringComparison]::Ordinal)
if ([string]::IsNullOrWhiteSpace($reminderOperationBinding) -or $reminderStateUpdateIndex -lt 0 -or
    $reminderSynchronizeIndex -le $reminderStateUpdateIndex) {
    throw 'TM-APP-REMINDER-SETTINGS-FIRST: the single operation worker must persist reminder settings before synchronization'
}
$reminderSettingsFirstBindingCount = 1
$replaceableCoordinator = [regex]::Match(
    $commandText,
    '(?s)pub\(crate\) fn submit_replaceable\(.*?\r?\n    \}\r?\n\r?\n    pub\(crate\) fn retry_last'
).Value
if ([string]::IsNullOrWhiteSpace($replaceableCoordinator) -or
    $replaceableCoordinator -notmatch 'active\.pending = Some\(PendingCommand \{ id, command \}\)' -or
    $replaceableCoordinator -notmatch 'ApplicationCommandAdmission::Coalesced' -or
    [regex]::Matches($operationText, 'state\.coordinator\.submit_replaceable\(command\)').Count -ne 1 -or
    [regex]::Matches($operationText, 'pending\.payload = payload;').Count -ne 1) {
    throw 'TM-APP-REMINDER-LATEST-WINS: policy updates require one active plus one replaceable pending payload'
}
$reminderLatestWinsBindingCount = 1
if ([regex]::Matches($applicationText, 'publish_pending_reminder_policy\(').Count -ne 3 -or
    [regex]::Matches($applicationText, 'publish_pending_reminder_operation\(').Count -ne 3) {
    throw 'TM-APP-REMINDER-VISIBLE-PENDING: Save and confirmed import must use the visible Pending acknowledgement path'
}
$reminderImportCommitFunction = [regex]::Match(
    $stateText,
    '(?s)pub\(crate\) fn commit_pending_config_import\(.*?\r?\n    \}\r?\n\r?\n    pub\(crate\) fn cancel_pending_config_import'
).Value
$importPendingIndex = $reminderImportCommitFunction.IndexOf('.swap(REMINDER_SYNC_PENDING, Ordering::AcqRel)', [System.StringComparison]::Ordinal)
$importVisibleIndex = $reminderImportCommitFunction.IndexOf('on_irreversible().is_err()', [System.StringComparison]::Ordinal)
$importCommitIndex = $reminderImportCommitFunction.IndexOf('.commit_import(&preview.settings)', [System.StringComparison]::Ordinal)
if ([string]::IsNullOrWhiteSpace($reminderImportCommitFunction) -or $importPendingIndex -lt 0 -or
    $importVisibleIndex -le $importPendingIndex -or $importCommitIndex -le $importVisibleIndex) {
    throw 'TM-APP-REMINDER-VISIBLE-PENDING: confirmed import must visibly publish Pending before committing settings'
}
$reminderVisiblePendingBindingCount = 2
$reminderImportBindingCount = [regex]::Matches(
    $applicationText,
    '(?s)\(ApplicationCommand::ConfirmConfigImport, ApplicationOperationPayload::Empty\)\s*=>\s*\{\s*match state\.commit_pending_config_import\(permit,.*?\}\)\s*\{\s*Ok\(_\)\s*=>\s*execute_state_command\(synchronize_reminder_policy_after_settings\('
).Count
$reminderStartupBindingCount = [regex]::Matches(
    $applicationText,
    '(?s)let reminder = start_optional_reminder_runtime\(\s*data_root,\s*state,\s*archive_path\.clone\(\),\s*started\.notifier_port\.clone\(\),\s*\);'
).Count
if ($reminderImportBindingCount -ne 1 -or $reminderStartupBindingCount -ne 1) {
    throw 'TM-APP-REMINDER-IMPORT-BINDING: startup and confirmed config import must share the sole reminder synchronizer'
}
$reminderStartupFunction = [regex]::Match(
    $applicationText,
    '(?s)fn start_optional_reminder_runtime\(.*?\r?\n\}\r?\n\r?\nfn begin_bundle_generation'
).Value
if ([string]::IsNullOrWhiteSpace($reminderStartupFunction) -or
    [regex]::Matches($reminderStartupFunction, 'state\.synchronize_reminder_profile\(data_root\)').Count -ne 1 -or
    [regex]::Matches($reminderStartupFunction, 'Err\(_\) => OptionalReminderRuntime::failed\(RuntimeErrorCode::StoreUnavailable\)').Count -ne 1 -or
    $reminderStartupFunction -match 'mark_reminder_unavailable|REMINDER_SYNC_UNAVAILABLE') {
    throw 'TM-APP-REMINDER-STARTUP-PENDING: startup store unavailability must retain the durable desired policy as retryable Pending'
}
$reminderStartupPendingBindingCount = 1
if ($notificationText -notmatch 'const NOTIFICATION_ACK_RETRY: Duration = Duration::from_secs\(60\);' -or
    $notificationText -notmatch 'Err\(error\) if error\.retryable\(\)' -or
    $notificationText -notmatch 'matches!\(self, Self::Busy \| Self::StoreUnavailable\)') {
    throw 'TM-APP-NOTIFICATION-RETRY: only Busy and StoreUnavailable may retry after exactly 60 seconds'
}
if ([regex]::Matches($productionText, '\.acknowledge_notifications\(\)').Count -ne 1 -or
    [regex]::Matches($notificationText, '\.acknowledge_notifications\(\)').Count -ne 1) {
    throw 'TM-APP-NOTIFICATION-ACK-AUTHORITY: only the dedicated presentation port may acknowledge reminders'
}
$acknowledgeWorkerFunction = [regex]::Match(
    $notificationText,
    '(?s)fn acknowledge_presented\(.*?\r?\n\}\r?\n\r?\nfn is_stopping'
).Value
if ([string]::IsNullOrWhiteSpace($acknowledgeWorkerFunction) -or
    $acknowledgeWorkerFunction -notmatch 'Err\(_\) => \{\s*let _ = release_with_retry\(signal, port, retry\);' -or
    $acknowledgeWorkerFunction -match 'release_then_retry_presentation') {
    throw 'TM-APP-NOTIFICATION-TERMINAL-ACK: terminal acknowledgement failure must release without re-presentation'
}
if ($notificationText -notmatch 'ReceiptAction::Failed => \{\s*release_then_retry_presentation\(' -or
    $notificationText -notmatch 'Err\(_\) => \{\s*let _ = release_with_retry\(signal, port, retry\);' -or
    $notificationText -notmatch 'let released = release_in_flight\(&self\.signal, self\.port\.as_ref\(\)\)') {
    throw 'TM-APP-NOTIFICATION-RELEASE: callback terminal and shutdown paths must release an outstanding lease'
}
$releaseFunction = [regex]::Match(
    $notificationText,
    '(?s)fn release_in_flight\(.*?\r?\n\}'
).Value
$runtimeReleaseIndex = $releaseFunction.IndexOf('if !port.release()?', [System.StringComparison]::Ordinal)
$localClearIndex = $releaseFunction.IndexOf('signal.clear_in_flight();', [System.StringComparison]::Ordinal)
if ([string]::IsNullOrWhiteSpace($releaseFunction) -or $runtimeReleaseIndex -lt 0 -or
    $localClearIndex -le $runtimeReleaseIndex -or
    $releaseFunction -notmatch 'if !port\.release\(\)\? \{\s*return Err\(PresentationFailure::Internal\);\s*\}') {
    throw 'TM-APP-NOTIFICATION-RELEASE-ORDER: local backpressure must remain until runtime release completes'
}
$retryPresentationFunction = [regex]::Match(
    $notificationText,
    '(?s)fn release_then_retry_presentation\(.*?\r?\n\}'
).Value
if ([string]::IsNullOrWhiteSpace($retryPresentationFunction) -or
    $retryPresentationFunction -notmatch 'wait_for_presentation_retry_or_action\(signal, retry\)' -or
    $retryPresentationFunction -notmatch 'pump_presentation\(signal, port, presenter\.as_ref\(\)\)') {
    throw 'TM-APP-NOTIFICATION-REPUMP: a released presentation must retry on the existing bounded worker'
}
$publishRuntime = [regex]::Match(
    $applicationText,
    '(?s)fn publish_runtime\(.*?\r?\n    \}\r?\n\r?\n    fn shutdown'
).Value
$presentationPumpIndex = $publishRuntime.IndexOf('presentation.pump()', [System.StringComparison]::Ordinal)
$controllerObservationIndex = $publishRuntime.IndexOf('.observe_runtime(observation)', [System.StringComparison]::Ordinal)
if ([string]::IsNullOrWhiteSpace($publishRuntime) -or $presentationPumpIndex -lt 0 -or
    $controllerObservationIndex -le $presentationPumpIndex) {
    throw 'TM-APP-NOTIFICATION-PUMP-ORDER: presentation must not depend on controller publication success'
}
$runtimeAcknowledge = [regex]::Match(
    $reminderRuntimeText,
    '(?s)fn acknowledge_notifications_with<.*?\r?\n    \}\r?\n\r?\n    pub fn release_notifications'
).Value
$runtimeBeginIndex = $runtimeAcknowledge.IndexOf('.begin_acknowledgement()', [System.StringComparison]::Ordinal)
$runtimeCatchIndex = $runtimeAcknowledge.IndexOf('std::panic::catch_unwind', [System.StringComparison]::Ordinal)
$runtimeFinishIndex = $runtimeAcknowledge.IndexOf('.finish_acknowledgement(committed)', [System.StringComparison]::Ordinal)
if ([string]::IsNullOrWhiteSpace($runtimeAcknowledge) -or $runtimeBeginIndex -lt 0 -or
    $runtimeCatchIndex -le $runtimeBeginIndex -or $runtimeFinishIndex -le $runtimeCatchIndex -or
    $runtimeAcknowledge -notmatch 'let committed = acknowledgement\.is_ok\(\);' -or
    $reminderRuntimeText -notmatch 'REDACT_REMINDER_RUNTIME_PANIC') {
    throw 'TM-APP-NOTIFICATION-PANIC-ROLLBACK: acknowledgement panic must restore the leased batch without exposing its payload'
}
if ($notificationText -notmatch '\.unwrap_or_else\(std::sync::PoisonError::into_inner\)\s*\.release_notifications\(\)') {
    throw 'TM-APP-NOTIFICATION-POISON-RELEASE: fallback release must recover only the outer runtime mutex'
}
if ($notificationText -notmatch 'struct ReceiptWorkerState \{\s*action: Option<ReceiptAction>,\s*stopping: bool,' -or
    $notificationText -notmatch 'in_flight: AtomicBool' -or
    [regex]::Matches($notificationText, 'compare_exchange\(false, true, Ordering::AcqRel, Ordering::Acquire\)').Count -lt 2 -or
    $notificationText -notmatch 'completed: AtomicBool') {
    throw 'TM-APP-NOTIFICATION-CAPACITY: presentation must keep one lease action and one one-shot receipt'
}
$bundleShutdown = [regex]::Match(
    $applicationText,
    '(?s)impl ApplicationBundle \{.*?fn shutdown\(&mut self\).*?\r?\n\}\r?\n\r?\nfn remember_failure'
).Value
$presentationShutdownIndex = $bundleShutdown.IndexOf(
    'self.notification_presentation.take()',
    [System.StringComparison]::Ordinal
)
$reminderPauseIndex = $bundleShutdown.IndexOf('reminder.pause()', [System.StringComparison]::Ordinal)
$reminderShutdownIndex = $bundleShutdown.IndexOf('reminder.shutdown()', [System.StringComparison]::Ordinal)
if ([string]::IsNullOrWhiteSpace($bundleShutdown) -or $presentationShutdownIndex -lt 0 -or
    $reminderPauseIndex -le $presentationShutdownIndex -or
    $reminderShutdownIndex -le $presentationShutdownIndex) {
    throw 'TM-APP-NOTIFICATION-SHUTDOWN-ORDER: notification bridge and worker must close before reminder lifecycle shutdown'
}
if ($productionText -match '\bstd::env::(args|args_os|current_dir|set_current_dir)\b') {
    throw 'TM-APP-ARBITRARY-ROOT: command-line or working-directory data roots are forbidden'
}
$environmentNames = @(
    [regex]::Matches($dataRootText, 'var_os\("([A-Z_]+)"\)') |
        ForEach-Object { $_.Groups[1].Value } |
        Sort-Object -Unique
)
$expectedEnvironmentNames = @('CODEX_HOME', 'LOCALAPPDATA', 'USERPROFILE')
if ($environmentNames.Count -ne $expectedEnvironmentNames.Count -or
    @($expectedEnvironmentNames | Where-Object { $_ -notin $environmentNames }).Count -ne 0) {
    throw "TM-APP-ARBITRARY-ROOT: environment surface drifted: $($environmentNames -join ', ')"
}
$authorityText = $productionText -replace `
    '(?m)^\s*use\s+std\s*::\s*process\s*::\s*ExitCode\s*;\s*$', `
    ''
if ($authorityText -match 'https?://|\bstd\s*::\s*process\b|\bprocess\s*::|\buse\s+std\s*::\s*\{[^;]*\bprocess\b|\bCommand\s*::\s*new\b|\b(TcpStream|TcpListener|UdpSocket)\b|\b(rusqlite|notify|reqwest|ureq|webbrowser|headless_chrome)\b|\bSELECT\s+[^;\r\n]+\s+FROM\b|\bINSERT\s+INTO\b|\bUPDATE\s+[A-Za-z_][A-Za-z0-9_]*\s+SET\b|\bDELETE\s+FROM\b|\bPRAGMA\s+[A-Za-z_]|powershell(?:\.exe)?|cmd(?:\.exe)?|bash(?:\.exe)?|\bsh\s+-c\b|\bAuthorization\b|\bBearer\s') {
    throw 'TM-APP-FORBIDDEN-AUTHORITY: composition contains network/shell/SQL/browser/credential authority'
}
if ($productionText -match '\b(WhereMyTokens|WhereMyToken|WhereMyTokensGo|ccusage-go)\b') {
    throw 'TM-APP-OLD-PROJECT: production composition contains an old project identity'
}
if ($dataRootText -notmatch 'ValidatedLocalDirectory::new' -or
    $dataRootText -notmatch 'fs::create_dir\(' -or
    $dataRootText -match 'create_dir_all|\.join\("portable"\)') {
    throw 'TM-APP-DATA-ROOT: exact one-child validated data-root policy drifted'
}

if ($SourceOnly) {
    [ordered]@{
        result = 'pass'
        scope = 'source-only'
        rust_source_file_count = $rustFiles.Count
        production_binary_owner_count = 1
        application_state_owner_count = 1
        application_preflight_count = 1
        live_runtime_owner_count = 1
        maintenance_runtime_owner_count = 1
        application_command_coordinator_count = 1
        application_operation_worker_count = 1
        application_operation_capacity_one_wake_count = 1
        application_operation_owned_spawn_count = 1
        application_backup_command_binding_count = 1
        application_operation_join_count = 1
        application_config_sealed_target_count = 1
        application_config_sealed_source_count = 1
        application_config_bounded_stage_count = 1
        application_config_bounded_read_count = 1
        controlled_restart_count = 1
        bundle_generation_guard_count = 1
        selected_restore_count = 2
        protected_pre_restore_count = 2
        dynamic_restore_pin_count = 1
        recovery_launch_binding_count = 2
        restored_migration_lifecycle_count = 2
        pre_migration_gate_count = 2
        post_migration_gate_count = 2
        pending_migration_transition_count = 2
        completed_migration_transition_count = 2
        atomic_maintenance_wait_count = 2
        clean_state_transition_count = 1
        quota_runtime_owner_count = 1
        reminder_runtime_owner_count = 1
        notification_receipt_worker_count = $notificationWorkerCount
        notification_ack_retry_seconds = 60
        notification_presentation_coordinator_count = 1
        notification_runtime_ack_authority_count = 1
        notification_confirmed_release_count = 1
        notification_bounded_repump_count = 1
        notification_runtime_panic_rollback_count = 1
        reminder_sealed_payload_count = $reminderSealedPayloadCount
        reminder_generation_binding_count = 1
        reminder_settings_first_binding_count = $reminderSettingsFirstBindingCount
        reminder_latest_wins_binding_count = $reminderLatestWinsBindingCount
        reminder_visible_pending_binding_count = $reminderVisiblePendingBindingCount
        reminder_import_binding_count = $reminderImportBindingCount
        reminder_startup_binding_count = $reminderStartupBindingCount
        reminder_startup_pending_binding_count = $reminderStartupPendingBindingCount
        desktop_controller_count = 1
        session_detail_router_count = 1
        session_detail_current_bundle_binding_count = 1
        session_detail_nonblocking_binding_count = 1
        desktop_bridge_count = 1
        application_polling_surface_count = 0
        arbitrary_root_surface_count = 0
        recovery_adversarial_anchor_count = $recoveryAdversarialAnchors.Count
    } | ConvertTo-Json -Compress
    return
}

$metadataJson = & cargo +1.97.0 metadata --locked --format-version 1 --manifest-path $rootManifest
if ($LASTEXITCODE -ne 0) {
    throw 'TM-APP-METADATA: cargo metadata failed'
}
$metadata = $metadataJson | ConvertFrom-Json -Depth 100
$appPackages = @($metadata.packages | Where-Object { $_.name -eq 'tokenmaster-app' })
if ($appPackages.Count -ne 1) {
    throw 'TM-APP-PACKAGE: tokenmaster-app must resolve exactly once'
}
$directProductionDependencies = @(
    $appPackages[0].dependencies |
        Where-Object { $null -eq $_.kind } |
        ForEach-Object { $_.name } |
        Sort-Object -Unique
)
$expectedDependencies = @(
    'slint', 'tokenmaster-codex', 'tokenmaster-desktop', 'tokenmaster-domain', 'tokenmaster-engine',
    'tokenmaster-platform', 'tokenmaster-product', 'tokenmaster-runtime',
    'tokenmaster-state', 'tokenmaster-store'
)
if ($directProductionDependencies.Count -ne $expectedDependencies.Count -or
    @($expectedDependencies | Where-Object { $_ -notin $directProductionDependencies }).Count -ne 0) {
    throw "TM-APP-DEPENDENCIES: direct dependency set drifted: $($directProductionDependencies -join ', ')"
}
$tokenMasterTargets = @(
    $metadata.packages | ForEach-Object {
        $package = $_
        $_.targets | Where-Object { $_.kind -contains 'bin' -and $_.name -eq 'TokenMaster' } |
            ForEach-Object { [pscustomobject]@{ Package = $package.name; Source = $_.src_path } }
    }
)
if ($tokenMasterTargets.Count -ne 1 -or $tokenMasterTargets[0].Package -ne 'tokenmaster-app') {
    throw 'TM-APP-DUPLICATE-BINARY: exactly one TokenMaster target must be owned by tokenmaster-app'
}

$featureTree = (& cargo +1.97.0 tree -p tokenmaster-app -e features --manifest-path $rootManifest) -join "`n"
if ($LASTEXITCODE -ne 0) {
    throw 'TM-APP-TREE: cargo feature tree failed'
}
if ($featureTree -notmatch 'renderer-software' -or $featureTree -match 'renderer-femtovg|tokenmaster-m0') {
    throw 'TM-APP-RENDERER: production tree must contain software renderer and no probe/FemtoVG'
}

& cargo +1.97.0 build --release --locked --manifest-path $rootManifest -p tokenmaster-app
if ($LASTEXITCODE -ne 0) {
    throw 'TM-APP-BUILD: release application build failed'
}
$targetDirectory = [System.IO.Path]::GetFullPath([string]$metadata.target_directory)
$artifacts = @(
    Get-ChildItem -LiteralPath $targetDirectory -Recurse -File -Filter 'TokenMaster.exe' |
        Where-Object { $_.FullName -match '[\\/]release[\\/]TokenMaster\.exe$' }
)
if ($artifacts.Count -ne 1) {
    throw 'TM-APP-ARTIFACT: release TokenMaster executable was not found'
}
$artifact = $artifacts[0].FullName
$artifactText = [System.Text.Encoding]::ASCII.GetString(
    [System.IO.File]::ReadAllBytes($artifact)
)
foreach ($needle in @(
    'seed_probe_models', 'TokenMaster M0', 'demo-session-', 'WhereMyTokensGo',
    'PRIVATE_GIT_RUNTIME_REPOSITORY', 'PRIVATE_SESSION_NAME.jsonl',
    'PIPELINE_PRIVATE_SENTINEL_91A7', 'PRIVATE_PARENT_MARKER',
    'Private@Example.com', 'credit_private_76e5', 'C:\private\codex-home',
    'Authorization: Bearer', 'auth.json'
)) {
    if ($artifactText.IndexOf($needle, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "TM-APP-BINARY-STRING: release executable contains forbidden string: $needle"
    }
}

[ordered]@{
    result = 'pass'
    package = 'tokenmaster-app'
    binary = 'TokenMaster.exe'
    direct_production_dependencies = $directProductionDependencies
    rust_source_file_count = $rustFiles.Count
    production_binary_owner_count = 1
    application_state_owner_count = 1
    application_preflight_count = 1
    live_runtime_owner_count = 1
    maintenance_runtime_owner_count = 1
    application_command_coordinator_count = 1
    application_operation_worker_count = 1
    application_operation_capacity_one_wake_count = 1
    application_operation_owned_spawn_count = 1
    application_backup_command_binding_count = 1
    application_operation_join_count = 1
    application_config_sealed_target_count = 1
    application_config_sealed_source_count = 1
    application_config_bounded_stage_count = 1
    application_config_bounded_read_count = 1
    controlled_restart_count = 1
    bundle_generation_guard_count = 1
    selected_restore_count = 2
    protected_pre_restore_count = 2
    dynamic_restore_pin_count = 1
    recovery_launch_binding_count = 2
    restored_migration_lifecycle_count = 2
    pre_migration_gate_count = 2
    post_migration_gate_count = 2
    pending_migration_transition_count = 2
    completed_migration_transition_count = 2
    atomic_maintenance_wait_count = 2
    clean_state_transition_count = 1
    quota_runtime_owner_count = 1
    reminder_runtime_owner_count = 1
    notification_receipt_worker_count = $notificationWorkerCount
    notification_ack_retry_seconds = 60
    notification_presentation_coordinator_count = 1
    notification_runtime_ack_authority_count = 1
    notification_confirmed_release_count = 1
    notification_bounded_repump_count = 1
    notification_runtime_panic_rollback_count = 1
    reminder_sealed_payload_count = $reminderSealedPayloadCount
    reminder_generation_binding_count = 1
    reminder_settings_first_binding_count = $reminderSettingsFirstBindingCount
    reminder_latest_wins_binding_count = $reminderLatestWinsBindingCount
    reminder_visible_pending_binding_count = $reminderVisiblePendingBindingCount
    reminder_import_binding_count = $reminderImportBindingCount
    reminder_startup_binding_count = $reminderStartupBindingCount
    reminder_startup_pending_binding_count = $reminderStartupPendingBindingCount
    desktop_controller_count = 1
    session_detail_router_count = 1
    session_detail_current_bundle_binding_count = 1
    session_detail_nonblocking_binding_count = 1
    desktop_bridge_count = 1
    application_polling_surface_count = 0
    arbitrary_root_surface_count = 0
    femtovg_feature_count = 0
    probe_dependency_count = 0
    release_artifact_count = 1
    forbidden_binary_string_count = 0
    recovery_adversarial_anchor_count = $recoveryAdversarialAnchors.Count
} | ConvertTo-Json -Compress
