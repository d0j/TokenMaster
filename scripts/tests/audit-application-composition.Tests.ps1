Describe "TokenMaster application composition audit" {
    BeforeAll {
        $ScriptsRoot = Split-Path -Parent $PSScriptRoot
        $RepositoryRoot = (Resolve-Path (Join-Path $ScriptsRoot "..")).Path
        $Audit = Join-Path $ScriptsRoot "audit-application-composition.ps1"

        function New-AppAuditFixture {
            param([Parameter(Mandatory = $true)][string]$Name)

            $fixture = Join-Path $TestDrive $Name
            New-Item -ItemType Directory -Path $fixture -Force | Out-Null
            Copy-Item -LiteralPath (Join-Path $RepositoryRoot "Cargo.toml") -Destination $fixture
            $crateParent = Join-Path $fixture "crates"
            New-Item -ItemType Directory -Path $crateParent -Force | Out-Null
            Copy-Item -LiteralPath (Join-Path $RepositoryRoot "crates\app") `
                -Destination $crateParent -Recurse
            Copy-Item -LiteralPath (Join-Path $RepositoryRoot "crates\desktop") `
                -Destination $crateParent -Recurse
            return $fixture
        }
    }

    It "accepts the current allowlisted ExitCode composition" {
        $fixture = New-AppAuditFixture -Name "current-composition"

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Not -Throw
    }

    It "rejects a second live runtime owner" {
        $fixture = New-AppAuditFixture -Name "duplicate-live"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\application.rs") `
            -Value 'fn duplicate_live() { let _ = LiveRuntime::start_notified_guarded('

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-LIVE-OWNER*"
    }

    It "rejects an unguarded live runtime owner" {
        $fixture = New-AppAuditFixture -Name "unguarded-live"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'LiveRuntime::start_notified_guarded(',
            'LiveRuntime::start_notified('
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-UNGUARDED-LIVE*"
    }

    It "rejects a second reliable state owner" {
        $fixture = New-AppAuditFixture -Name "duplicate-state-owner"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\application.rs") `
            -Value 'fn duplicate_state_owner() { let _ = ApplicationStateOwner::open('

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-STATE-OWNER*"
    }

    It "rejects a second application preflight" {
        $fixture = New-AppAuditFixture -Name "duplicate-preflight"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\application.rs") `
            -Value 'fn duplicate_preflight() { let _ = owner.prepare(&data_root); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-PREFLIGHT*"
    }

    It "rejects a second backup maintenance runtime owner" {
        $fixture = New-AppAuditFixture -Name "duplicate-maintenance-owner"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\state.rs") `
            -Value 'fn duplicate_maintenance() { let _ = BackupMaintenanceRuntime::spawn('

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-MAINTENANCE-OWNER*"
    }

    It "rejects a second application command coordinator" {
        $fixture = New-AppAuditFixture -Name "duplicate-command-coordinator"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\application.rs") `
            -Value 'fn duplicate_commands() { let _ = ApplicationCommandCoordinator::new(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-COMMAND-COORDINATOR*"
    }

    It "rejects a second application operation worker" {
        $fixture = New-AppAuditFixture -Name "duplicate-operation-worker"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\application.rs") `
            -Value 'fn duplicate_operation_worker() { let _ = ApplicationOperationWorker::spawn('

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-OPERATION-WORKER*"
    }

    It "rejects an unbounded application operation wake" {
        $fixture = New-AppAuditFixture -Name "unbounded-operation-wake"
        $path = Join-Path $fixture "crates\app\src\operation.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'sync_channel(1)',
            'channel()'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-OPERATION-WAKE*"
    }

    It "rejects a second application operation thread builder" {
        $fixture = New-AppAuditFixture -Name "duplicate-operation-thread-builder"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\operation.rs") `
            -Value 'fn detached_operation_thread() { let _ = Builder::new(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-OPERATION-SPAWN*"
    }

    It "rejects replacing the sealed config export target" {
        $fixture = New-AppAuditFixture -Name "unsealed-config-target"
        $path = Join-Path $fixture "crates\app\src\state.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'target: &DurableFileTarget',
            'target: &std::path::Path'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-CONFIG-SEALED-TARGET*"
    }

    It "rejects removing the config import read ceiling" {
        $fixture = New-AppAuditFixture -Name "unbounded-config-read"
        $path = Join-Path $fixture "crates\app\src\state.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '.open_reader(MAX_CONFIG_PACKAGE_BYTES)',
            '.open_reader(MAX_DURABLE_FILE_BYTES)'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-CONFIG-BOUNDED-READ*"
    }

    It "rejects removal of the manual backup command binding" {
        $fixture = New-AppAuditFixture -Name "backup-command-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'ApplicationCommand::Backup => execute_manual_backup_command(',
            'ApplicationCommand::Backup => execute_unbound_backup('
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-BACKUP-COMMAND*"
    }

    It "rejects detaching the application operation worker at shutdown" {
        $fixture = New-AppAuditFixture -Name "operation-join-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'self.commands.shutdown()',
            'self.commands.detach()'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-OPERATION-JOIN*"
    }

    It "rejects removal of restart admission closure" {
        $fixture = New-AppAuditFixture -Name "restart-admission-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '.pause_admission()',
            '.leave_admission_open()'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-RESTART-PAUSE*"
    }

    It "rejects removal of the fresh restart lease guard" {
        $fixture = New-AppAuditFixture -Name "restart-guard-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '.acquire_runtime_guard(&self.data_root)',
            '.reuse_obsolete_runtime_guard(&self.data_root)'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-RESTART-GUARD*"
    }

    It "rejects ordinal-only selected restore" {
        $fixture = New-AppAuditFixture -Name "restore-binding-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '.bind_backup_selection(selection)',
            '.trust_backup_ordinal(selection)'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-RESTORE-BINDING*"
    }

    It "rejects binding against a stale directory projection" {
        $fixture = New-AppAuditFixture -Name "restore-current-binding-drift"
        $path = Join-Path $fixture "crates\app\src\state.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '.bind_current_selection(&self.backups, point.selection())',
            '.bind_selection(point.selection())'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-RESTORE-CURRENT-BIND*"
    }

    It "rejects deleting without consulting the late restore pin" {
        $fixture = New-AppAuditFixture -Name "restore-dynamic-pin-drift"
        $path = Join-Path $fixture "crates\app\src\state.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'retention.delete_next_protected(',
            'retention.delete_next_unprotected('
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-RESTORE-DYNAMIC-PIN*"
    }

    It "rejects leaking the process-local restore pin" {
        $fixture = New-AppAuditFixture -Name "restore-pin-drop-drift"
        $path = Join-Path $fixture "crates\app\src\state.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'impl Drop for ApplicationBackupSelectionPin',
            'impl Leak for ApplicationBackupSelectionPin'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-RESTORE-PIN-DROP*"
    }

    It "rejects unprotected pre-restore maintenance" {
        $fixture = New-AppAuditFixture -Name "restore-protection-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '.start_protected_maintenance(',
            '.start_maintenance('
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-RESTORE-PROTECTED*"
    }

    It "rejects dropping the selected recovery receipt" {
        $fixture = New-AppAuditFixture -Name "restore-receipt-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '.bind_recovery_launch(receipt)',
            '.discard_recovery_launch(receipt)'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-RESTORE-RECOVERY-LAUNCH*"
    }

    It "rejects binding the recovery receipt after restored lifecycle work" {
        $fixture = New-AppAuditFixture -Name "restore-receipt-order-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'self.preflight.bind_recovery_launch(receipt)?;',
            ''
        )
        $text += "`nfn bind_too_late() { self.preflight.bind_recovery_launch(receipt)?; }`n"
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-RESTORE-RECOVERY-ORDER*"
    }

    It "rejects bypassing restored-archive migration gates" {
        $fixture = New-AppAuditFixture -Name "restored-migration-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'start_restored_bundle(',
            'start_current_bundle('
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-RESTORED-MIGRATION*"
    }

    It "rejects migration safety-point drift" {
        $fixture = New-AppAuditFixture -Name "migration-gate-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'MaintenancePurpose::PostMigration',
            'MaintenancePurpose::Manual'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-POST-MIGRATION*"
    }

    It "rejects removal of the durable pending migration transition" {
        $fixture = New-AppAuditFixture -Name "pending-migration-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '.require_post_migration(',
            '.forget_post_migration('
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-MIGRATION-PENDING*"
    }

    It "rejects removal of the completed migration transition" {
        $fixture = New-AppAuditFixture -Name "complete-migration-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '.complete_post_migration(',
            '.leave_post_migration_pending('
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-MIGRATION-COMPLETE*"
    }

    It "rejects splitting mandatory submission from its exact-root wait" {
        $fixture = New-AppAuditFixture -Name "atomic-wait-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '.submit_and_wait(',
            '.submit_then_poll('
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-ATOMIC-MAINTENANCE-WAIT*"
    }

    It "rejects a second clean-state transition" {
        $fixture = New-AppAuditFixture -Name "duplicate-clean"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\application.rs") `
            -Value 'fn duplicate_clean() { let _ = session.mark_clean(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-CLEAN-STATE*"
    }

    It "rejects polling threads and timers" {
        $fixture = New-AppAuditFixture -Name "polling"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\application.rs") `
            -Value 'fn polling() { std::thread::spawn(|| {}); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-POLLING*"
    }

    It "rejects command-line or working-directory roots" {
        $fixture = New-AppAuditFixture -Name "arbitrary-root"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\data_root.rs") `
            -Value 'fn cwd_root() { let _ = std::env::current_dir(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-ARBITRARY-ROOT*"
    }

    It "rejects portable marker drift" {
        $fixture = New-AppAuditFixture -Name "marker"
        $path = Join-Path $fixture "crates\app\src\data_root.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '"tokenmaster.portable"',
            '"portable.mode"'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-PORTABLE-MARKER*"
    }

    It "rejects a strong notifier ownership cycle" {
        $fixture = New-AppAuditFixture -Name "strong-notifier"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'Weak<Mutex<ApplicationBundleSlot>>',
            'Arc<Mutex<ApplicationBundleSlot>>'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-WEAK-NOTIFIER*"
    }

    It "rejects removal of obsolete bundle generation suppression" {
        $fixture = New-AppAuditFixture -Name "obsolete-bundle-generation"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'slot.generation != self.bundle_generation',
            'false'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-BUNDLE-GENERATION*"
    }

    It "rejects probe dependencies" {
        $fixture = New-AppAuditFixture -Name "probe"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\Cargo.toml") `
            -Value 'tokenmaster-m0 = { path = "../probe-app" }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-PROBE-DEPENDENCY*"
    }

    It "rejects shell network SQL browser and credential surfaces" {
        $fixture = New-AppAuditFixture -Name "forbidden-authority"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\application.rs") `
            -Value 'const PRIVATE_API: &str = "https://example.invalid";'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-FORBIDDEN-AUTHORITY*"
    }

    It "rejects grouped process command imports" {
        $fixture = New-AppAuditFixture -Name "grouped-process-command"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\command.rs") `
            -Value 'use std::process::{Command}; fn escaped_process() { let _ = Command::new("tool"); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-FORBIDDEN-AUTHORITY*"
    }

    It "rejects a second TokenMaster binary owner" {
        $fixture = New-AppAuditFixture -Name "duplicate-binary"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\Cargo.toml") `
            -Value "`r`n[[bin]]`r`nname = `"TokenMaster`"`r`npath = `"src/lib.rs`""

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-DUPLICATE-BINARY*"
    }
}
