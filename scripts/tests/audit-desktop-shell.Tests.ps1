Describe "TokenMaster production desktop audit" {
    BeforeAll {
        $ScriptsRoot = Split-Path -Parent $PSScriptRoot
        $RepositoryRoot = (Resolve-Path (Join-Path $ScriptsRoot "..")).Path
        $Audit = Join-Path $ScriptsRoot "audit-desktop-shell.ps1"

        function New-DesktopAuditFixture {
            param([Parameter(Mandatory = $true)][string]$Name)

            $fixture = Join-Path $TestDrive $Name
            New-Item -ItemType Directory -Path $fixture -Force | Out-Null
            Copy-Item -LiteralPath (Join-Path $RepositoryRoot "Cargo.toml") -Destination $fixture
            $crateParent = Join-Path $fixture "crates"
            New-Item -ItemType Directory -Path $crateParent -Force | Out-Null
            Copy-Item -LiteralPath (Join-Path $RepositoryRoot "crates\desktop") `
                -Destination $crateParent -Recurse
            return $fixture
        }
    }

    It "rejects probe dependencies" {
        $fixture = New-DesktopAuditFixture -Name "probe"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\Cargo.toml") `
            -Value 'tokenmaster-m0 = { path = "../probe-app" }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PROBE-DEPENDENCY*"
    }

    It "rejects mock or seeded production data" {
        $fixture = New-DesktopAuditFixture -Name "mock"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\lib.rs") `
            -Value "fn seed_probe_models() {}"

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-MOCK-DATA*"
    }

    It "rejects the diagnostic renderer" {
        $fixture = New-DesktopAuditFixture -Name "femtovg"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\Cargo.toml") `
            -Value 'slint = { workspace = true, features = ["renderer-femtovg"] }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-FEMTOVG*"
    }

    It "rejects route-count drift" {
        $fixture = New-DesktopAuditFixture -Name "routes"
        $path = Join-Path $fixture "crates\desktop\src\presentation.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'Self::CompactWidget => "compact_widget",',
            'Self::CompactWidget => "compact_widget_extra",'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-ROUTE-COUNT*"
    }

    It "rejects direct store or runtime authority" {
        $fixture = New-DesktopAuditFixture -Name "direct-authority"
        $path = Join-Path $fixture "crates\desktop\Cargo.toml"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '[build-dependencies]',
            "tokenmaster-store = { path = `"../store`" }`r`n`r`n[build-dependencies]"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DIRECT-AUTHORITY*"
    }

    It "rejects network browser shell SQL and filesystem surfaces" {
        $fixture = New-DesktopAuditFixture -Name "forbidden-authority"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\lib.rs") `
            -Value 'const PRIVATE_API: &str = "https://example.invalid";'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-FORBIDDEN-AUTHORITY*"
    }

    It "rejects a second desktop controller worker" {
        $fixture = New-DesktopAuditFixture -Name "controller-worker"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\controller.rs") `
            -Value 'fn extra_worker() { let _ = RefreshWorker::spawn('

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-CONTROLLER-WORKER*"
    }

    It "rejects query work from the Slint adapter" {
        $fixture = New-DesktopAuditFixture -Name "ui-query"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\ui.rs") `
            -Value 'fn callback_query() { let _ = DesktopController::refresh; }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-UI-QUERY*"
    }

    It "rejects exact-empty quota discovery from the dashboard controller" {
        $fixture = New-DesktopAuditFixture -Name "empty-filter-discovery"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\controller.rs") `
            -Value 'fn false_discovery() { let _ = QuotaCurrentRequest::new(Vec::new()); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-EMPTY-FILTER-DISCOVERY*"
    }

    It "rejects fixed five-hour or weekly quota rows" {
        $fixture = New-DesktopAuditFixture -Name "fixed-quota-row"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\main.slint") `
            -Value 'Text { text: "Weekly limit"; }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-FIXED-QUOTA-ROW*"
    }

    It "rejects seeded dashboard values in Slint" {
        $fixture = New-DesktopAuditFixture -Name "seeded-dashboard"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\main.slint") `
            -Value 'property <string> dashboard-header-tokens: "140";'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SEEDED-DASHBOARD*"
    }

    It "rejects private identity fields from the UI boundary" {
        $fixture = New-DesktopAuditFixture -Name "private-ui-identity"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\models.slint") `
            -Value 'export struct LeakyRow { account-id: string, session-id: string }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PRIVATE-IDENTITY*"
    }

    It "rejects UI timers or animations" {
        $fixture = New-DesktopAuditFixture -Name "ui-animation"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\main.slint") `
            -Value 'animate width { duration: 100ms; }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-UI-POLLING*"
    }

    It "rejects dashboard presentation-bound drift" {
        $fixture = New-DesktopAuditFixture -Name "dashboard-bound"
        $path = Join-Path $fixture "crates\desktop\src\dashboard.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'pub const MAX_DASHBOARD_TREND_POINTS: usize = 240;',
            'pub const MAX_DASHBOARD_TREND_POINTS: usize = 2400;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DASHBOARD-BOUND*"
    }

    It "rejects history presentation-bound drift" {
        $fixture = New-DesktopAuditFixture -Name "history-bound"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'pub const MAX_HISTORY_DAYS: usize = 30;',
            'pub const MAX_HISTORY_DAYS: usize = 300;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-BOUND*"
    }

    It "rejects restore-point presentation-bound drift" {
        $fixture = New-DesktopAuditFixture -Name "restore-bound"
        $path = Join-Path $fixture "crates\desktop\src\reliable_state.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'pub const MAX_DESKTOP_RESTORE_POINTS: usize = 15;',
            'pub const MAX_DESKTOP_RESTORE_POINTS: usize = 150;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-RESTORE-BOUND*"
    }

    It "rejects re-resolving restore identity after preview" {
        $fixture = New-DesktopAuditFixture -Name "restore-identity-drift"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'let selection = *reviewed_selection.borrow();',
            'let selection = None;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-RESTORE-IDENTITY*"
    }

    It "rejects fabricating zero values for unavailable reliable-state metrics" {
        $fixture = New-DesktopAuditFixture -Name "unknown-metrics-drift"
        $path = Join-Path $fixture "crates\desktop\src\reliable_state.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'successful_count: Option<u64>',
            'successful_count: u64'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-UNKNOWN-METRICS*"
    }

    It "rejects passphrase retention in Slint models" {
        $fixture = New-DesktopAuditFixture -Name "secret-model"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\models.slint") `
            -Value 'export struct SecretRow { passphrase: string }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SECRET-MODEL*"
    }

    It "rejects backup policy control range drift" {
        $fixture = New-DesktopAuditFixture -Name "policy-bound"
        $path = Join-Path $fixture "crates\desktop\ui\views\settings-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'minimum: 256; maximum: 65536',
            'minimum: 64; maximum: 32768'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-POLICY-BOUND*"
    }

    It "rejects dashboard model rebuilding from route selection" {
        $fixture = New-DesktopAuditFixture -Name "route-dashboard-rebuild"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'apply_route_projection(&window, state.projection());',
            "apply_route_projection(&window, state.projection());`r`n            apply_dashboard_projection(&window, state.projection().dashboard());"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DASHBOARD-REBUILD*"
    }

    It "rejects history model rebuilding from route selection" {
        $fixture = New-DesktopAuditFixture -Name "route-history-rebuild"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'apply_route_projection(&window, state.projection());',
            "apply_route_projection(&window, state.projection());`r`n            apply_history_projection(&window, state.projection().history());"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-REBUILD*"
    }

    It "rejects a second event-loop scheduling site" {
        $fixture = New-DesktopAuditFixture -Name "bridge-event"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\bridge.rs") `
            -Value 'fn extra_event() { let _ = slint::invoke_from_event_loop('

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-BRIDGE-EVENT*"
    }

    It "rejects a second reliable-state event-loop scheduling site" {
        $fixture = New-DesktopAuditFixture -Name "reliable-event"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\ui.rs") `
            -Value 'fn extra_reliable_event() { let _ = slint::invoke_from_event_loop('

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-RELIABLE-EVENT*"
    }

    It "rejects replacement of the reliable-state latest-only slot" {
        $fixture = New-DesktopAuditFixture -Name "reliable-slot"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'latest: Mutex<Option<DesktopReliableStateProjection>>',
            'latest: Mutex<Vec<DesktopReliableStateProjection>>'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-RELIABLE-SLOT*"
    }

    It "rejects hiding non-reconstructible loss after source rebuild" {
        $fixture = New-DesktopAuditFixture -Name "recovery-receipt-hidden"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'set_reliable_non_reconstructible_domains_lost',
            'discard_non_reconstructible_domains_lost'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-RECOVERY-RECEIPT*"
    }

    It "rejects a bridge polling thread or timer" {
        $fixture = New-DesktopAuditFixture -Name "bridge-polling"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\bridge.rs") `
            -Value 'fn polling() { std::thread::spawn(|| {}); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-BRIDGE-POLLING*"
    }

    It "rejects a strong Slint window in the bridge" {
        $fixture = New-DesktopAuditFixture -Name "bridge-strong-window"
        $path = Join-Path $fixture "crates\desktop\src\bridge.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'window: slint::Weak<MainWindow>',
            'window: MainWindow'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-BRIDGE-WEAK*"
    }

    It "rejects a second retained product snapshot slot" {
        $fixture = New-DesktopAuditFixture -Name "bridge-second-slot"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\bridge.rs") `
            -Value 'type ExtraSlot = Arc<Mutex<Option<Arc<ProductSnapshot>>>>;'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-CONTROLLER-SLOT*"
    }

    It "accepts the library-only bounded dashboard and history desktop boundary" {
        $fixture = New-DesktopAuditFixture -Name "library-boundary"

        $receipt = & $Audit -RepositoryRoot $fixture -SourceOnly | ConvertFrom-Json
        $receipt.rust_source_file_count | Should -Be 9
        $receipt.slint_source_file_count | Should -Be 15
        $receipt.dashboard_section_count | Should -Be 6
        $receipt.dashboard_model_replacement_count | Should -Be 7
        $receipt.history_day_maximum | Should -Be 30
        $receipt.history_model_replacement_count | Should -Be 1
        $receipt.history_projection_application_count | Should -Be 1
        $receipt.restore_point_maximum | Should -Be 15
        $receipt.restore_model_replacement_count | Should -Be 1
        $receipt.secret_model_count | Should -Be 0
        $receipt.event_loop_schedule_site_count | Should -Be 2
        $receipt.bridge_event_loop_schedule_site_count | Should -Be 1
        $receipt.reliable_event_loop_schedule_site_count | Should -Be 1
    }
}
