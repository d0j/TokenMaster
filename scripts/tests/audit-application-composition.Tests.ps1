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
            'Weak<Mutex<Option<ApplicationBundle>>>',
            'Arc<Mutex<Option<ApplicationBundle>>>'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-WEAK-NOTIFIER*"
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

    It "rejects a second TokenMaster binary owner" {
        $fixture = New-AppAuditFixture -Name "duplicate-binary"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\Cargo.toml") `
            -Value "`r`n[[bin]]`r`nname = `"TokenMaster`"`r`npath = `"src/lib.rs`""

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-DUPLICATE-BINARY*"
    }
}
