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

    It "rejects Models presentation-bound drift" {
        $fixture = New-DesktopAuditFixture -Name "models-bound"
        $path = Join-Path $fixture "crates\desktop\src\models.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'pub const MAX_MODEL_ROWS: usize = 64;',
            'pub const MAX_MODEL_ROWS: usize = 640;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-MODELS-BOUND*"
    }

    It "rejects a separate or incomplete Models analytics request" {
        $fixture = New-DesktopAuditFixture -Name "models-request"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'vec![UsageBreakdownKind::Model, UsageBreakdownKind::Project],',
            'vec![UsageBreakdownKind::Model],'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-MODELS-REQUEST*"
    }

    It "rejects a third analytics query for Models" {
        $fixture = New-DesktopAuditFixture -Name "models-third-query"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\controller.rs") `
            -Value 'fn third_models_query() { let _ = source.usage_analytics('

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-MODELS-REQUEST*"
    }

    It "rejects loss of a complete responsive Models token mix" {
        $fixture = New-DesktopAuditFixture -Name "models-view"
        $path = Join-Path $fixture "crates\desktop\ui\views\models-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'model.reasoning-label',
            '"reasoning hidden"'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-MODELS-VIEW*"
    }

    It "rejects loss of Models cost availability and provenance" {
        $fixture = New-DesktopAuditFixture -Name "models-cost-evidence"
        $path = Join-Path $fixture "crates\desktop\ui\views\models-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'model.cost-evidence-label',
            '"cost evidence hidden"'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-MODELS-VIEW*"
    }

    It "rejects Projects presentation-bound drift" {
        $fixture = New-DesktopAuditFixture -Name "projects-bound"
        $path = Join-Path $fixture "crates\desktop\src\projects.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'pub const MAX_PROJECT_ROWS: usize = 32;',
            'pub const MAX_PROJECT_ROWS: usize = 320;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PROJECTS-BOUND*"
    }

    It "rejects a second or non-today Projects Git query" {
        $fixture = New-DesktopAuditFixture -Name "projects-git-request"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\controller.rs") `
            -Value 'fn second_projects_git_query() { let _ = source.git_output('

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PROJECTS-REQUEST*"
    }

    It "rejects fuzzy project-to-Git alias matching" {
        $fixture = New-DesktopAuditFixture -Name "projects-fuzzy-join"
        $path = Join-Path $fixture "crates\desktop\src\projects.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'alias.as_str() == project',
            'alias.as_str().contains(project)'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PROJECTS-JOIN*"
    }

    It "rejects multiplying project cost by same-alias repository count" {
        $fixture = New-DesktopAuditFixture -Name "projects-cost-multiply"
        $path = Join-Path $fixture "crates\desktop\src\projects.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'self.cost = Some(cost);',
            'self.cost = self.cost.and_then(|current| current.checked_add(cost));'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PROJECTS-EFFICIENCY*"
    }

    It "rejects merging or hiding Projects usage and code ranges" {
        $fixture = New-DesktopAuditFixture -Name "projects-two-ranges"
        $path = Join-Path $fixture "crates\desktop\ui\views\projects-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'Today code',
            'Combined range'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PROJECTS-VIEW*"
    }

    It "rejects loss of complete responsive Projects Git evidence" {
        $fixture = New-DesktopAuditFixture -Name "projects-view"
        $path = Join-Path $fixture "crates\desktop\ui\views\projects-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'project.removed-label',
            '"removed hidden"'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PROJECTS-VIEW*"
    }

    It "rejects hiding Projects code availability and efficiency reasons" {
        $fixture = New-DesktopAuditFixture -Name "projects-status-reason"
        $path = Join-Path $fixture "crates\desktop\ui\views\projects-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'project.code-status-label',
            '"status hidden"'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PROJECTS-VIEW*"
    }

    It "rejects fabricating zero repositories for an unlinked Projects row" {
        $fixture = New-DesktopAuditFixture -Name "projects-not-linked"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '"repository_not_linked" => "Not linked".to_owned(),',
            '"repository_not_linked" => "0 repositories".to_owned(),'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PROJECTS-VIEW*"
    }

    It "rejects private identity fields from the Projects projection" {
        $fixture = New-DesktopAuditFixture -Name "projects-private-identity"
        $path = Join-Path $fixture "crates\desktop\src\projects.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'project: Arc<str>,',
            "project: Arc<str>,`r`n    repository_id: Arc<str>,"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PROJECTS-IDENTITY*"
    }

    It "rejects Activity presentation-bound drift" {
        $fixture = New-DesktopAuditFixture -Name "activity-bound"
        $path = Join-Path $fixture "crates\desktop\src\activity.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'pub const MAX_ACTIVITY_ROWS: usize = 12;',
            'pub const MAX_ACTIVITY_ROWS: usize = 120;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-ACTIVITY-BOUND*"
    }

    It "rejects a second Activity query" {
        $fixture = New-DesktopAuditFixture -Name "activity-second-query"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\controller.rs") `
            -Value 'fn duplicate_activity_query() { source.latest_activity(plan.activity); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-ACTIVITY-REQUEST*"
    }

    It "rejects removing the Activity route mount" {
        $fixture = New-DesktopAuditFixture -Name "activity-mount"
        $path = Join-Path $fixture "crates\desktop\ui\main.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'if root.activity-visible: ActivityView',
            'if root.activity-visible: RemovedActivityView'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-ACTIVITY-VIEW*"
    }

    It "rejects hiding reasoning from Activity rows" {
        $fixture = New-DesktopAuditFixture -Name "activity-reasoning"
        $path = Join-Path $fixture "crates\desktop\ui\views\activity-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'item.reasoning-label',
            '"reasoning hidden"'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-ACTIVITY-VIEW*"
    }

    It "rejects discarding fractional Activity timestamps" {
        $fixture = New-DesktopAuditFixture -Name "activity-fractional-time"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'row.timestamp_nanos()',
            '0'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-ACTIVITY-VIEW*"
    }

    It "rejects hiding a retained empty Activity page" {
        $fixture = New-DesktopAuditFixture -Name "activity-retained-empty"
        $path = Join-Path $fixture "crates\desktop\ui\views\activity-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'No activity events in the available page',
            'Recent activity evidence unavailable'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-ACTIVITY-VIEW*"
    }

    It "rejects private identity fields from the Activity projection" {
        $fixture = New-DesktopAuditFixture -Name "activity-private-identity"
        $path = Join-Path $fixture "crates\desktop\src\activity.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'model: Arc<str>,',
            "model: Arc<str>,`r`n    event_id: Arc<str>,"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-ACTIVITY-IDENTITY*"
    }

    It "rejects a second Activity model replacement site" {
        $fixture = New-DesktopAuditFixture -Name "activity-second-model"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\ui.rs") `
            -Value 'fn duplicate_activity_model() { window.set_recent_activity_rows(model(rows)); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-ACTIVITY-MODEL*"
    }

    It "rejects rebuilding Activity rows from route selection" {
        $fixture = New-DesktopAuditFixture -Name "activity-route-rebuild"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'apply_route_projection(&window, state.projection());',
            "apply_route_projection(&window, state.projection());`r`n            apply_activity_route_projection(&window, state.projection().activity());"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-ACTIVITY-REBUILD*"
    }

    It "rejects presenting Recent activity as a rhythm heatmap" {
        $fixture = New-DesktopAuditFixture -Name "activity-false-rhythm"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\views\activity-view.slint") `
            -Value '// activity rhythm heatmap'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-ACTIVITY-RHYTHM*"
    }

    It "rejects Notifications presentation-bound drift" {
        $fixture = New-DesktopAuditFixture -Name "notifications-bound"
        $path = Join-Path $fixture "crates\desktop\src\notifications.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'pub const MAX_NOTIFICATION_LOTS: usize = 256;',
            'pub const MAX_NOTIFICATION_LOTS: usize = 2560;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-NOTIFICATIONS-BOUND*"
    }

    It "rejects a second Notifications benefit query" {
        $fixture = New-DesktopAuditFixture -Name "notifications-second-query"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\controller.rs") `
            -Value 'fn duplicate_benefit_query() { source.benefit_overview(BenefitOverviewRequest::new()); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-NOTIFICATIONS-REQUEST*"
    }

    It "rejects removing the Notifications route mount" {
        $fixture = New-DesktopAuditFixture -Name "notifications-mount"
        $path = Join-Path $fixture "crates\desktop\ui\main.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'if root.notifications-visible: NotificationsView',
            'if root.notifications-visible: RemovedNotificationsView'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-NOTIFICATIONS-VIEW*"
    }

    It "rejects hiding expiry precision from Notifications rows" {
        $fixture = New-DesktopAuditFixture -Name "notifications-expiry"
        $path = Join-Path $fixture "crates\desktop\ui\views\notifications-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'lot.expiry-label',
            '"expiry hidden"'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-NOTIFICATIONS-VIEW*"
    }

    It "rejects collapsing uncertain Notifications expiry variants" {
        $fixture = New-DesktopAuditFixture -Name "notifications-expiry-precision"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'DesktopBenefitExpiry::ProviderLocal',
            'DesktopBenefitExpiry::Unknown'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-NOTIFICATIONS-VIEW*"
    }

    It "rejects private delivery identity from the Notifications projection" {
        $fixture = New-DesktopAuditFixture -Name "notifications-private-identity"
        $path = Join-Path $fixture "crates\desktop\src\notifications.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'scope_ordinal: u8,',
            "scope_ordinal: u8,`r`n    delivery_id: Arc<str>,"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-NOTIFICATIONS-IDENTITY*"
    }

    It "rejects direct reminder delivery authority from Notifications" {
        $fixture = New-DesktopAuditFixture -Name "notifications-delivery-authority"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\notifications.rs") `
            -Value 'fn false_delivery(runtime: &BenefitReminderRuntime) { let _ = runtime.acknowledge_notifications(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-NOTIFICATIONS-AUTHORITY*"
    }

    It "rejects a Notifications query or database owner" {
        $fixture = New-DesktopAuditFixture -Name "notifications-query-owner"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\notifications.rs") -Value 'fn false_query() { let _ = QueryService::open("private.sqlite3", FixedClock); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-NOTIFICATIONS-AUTHORITY*"
    }

    It "rejects a Notifications worker or thread owner" {
        $fixture = New-DesktopAuditFixture -Name "notifications-thread-owner"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\notifications.rs") -Value 'fn false_worker() { let _ = std::thread::spawn(|| {}); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-NOTIFICATIONS-AUTHORITY*"
    }

    It "rejects a Notifications queue or cache" {
        $fixture = New-DesktopAuditFixture -Name "notifications-queue-owner"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\notifications.rs") -Value 'struct FalseNotificationQueue { pending: VecDeque<String>, notification_cache: HashMap<String, String> }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-NOTIFICATIONS-AUTHORITY*"
    }

    It "rejects Notifications polling or a timer" {
        $fixture = New-DesktopAuditFixture -Name "notifications-polling-owner"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\notifications.rs") -Value 'fn poll_notifications() { let _ = Timer::default(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-NOTIFICATIONS-AUTHORITY*"
    }

    It "rejects a Notifications activation callback" {
        $fixture = New-DesktopAuditFixture -Name "notifications-activation-control"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\views\notifications-view.slint") -Value 'export component FalseActivation { callback activate-benefit(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-NOTIFICATIONS-AUTHORITY*"
    }

    It "rejects omitting profile completeness from the wide Notifications layout" {
        $fixture = New-DesktopAuditFixture -Name "notifications-wide-completeness"
        $path = Join-Path $fixture "crates\desktop\ui\views\notifications-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'Text { text: scope.completeness-label + " · " + scope.evidence-label; color: UiTokens.text-secondary; font-size: 10px; width: root.narrow ? 0px : 184px; visible: !root.narrow;',
            'Text { text: "Completeness hidden · " + scope.evidence-label; color: UiTokens.text-secondary; font-size: 10px; width: root.narrow ? 0px : 184px; visible: !root.narrow;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-NOTIFICATIONS-VIEW*"
    }

    It "rejects a second Notifications model replacement site" {
        $fixture = New-DesktopAuditFixture -Name "notifications-second-model"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\ui.rs") `
            -Value 'fn duplicate_notification_model() { window.set_benefit_lot_rows(model(lot_rows)); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-NOTIFICATIONS-MODEL*"
    }

    It "rejects rebuilding Notifications rows from route selection" {
        $fixture = New-DesktopAuditFixture -Name "notifications-route-rebuild"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'apply_route_projection(&window, state.projection());',
            "apply_route_projection(&window, state.projection());`r`n            apply_notifications_projection(&window, state.projection().notifications());"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-NOTIFICATIONS-REBUILD*"
    }

    It "rejects removing the Help About route mount" {
        $fixture = New-DesktopAuditFixture -Name "help-about-mount"
        $path = Join-Path $fixture "crates\desktop\ui\main.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'help-view := HelpAboutView',
            'help-view := RemovedHelpAboutView'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-VIEW*"
    }

    It "rejects conditional Help About reconstruction on route switches" {
        $fixture = New-DesktopAuditFixture -Name "help-about-conditional-lifecycle"
        $path = Join-Path $fixture "crates\desktop\ui\main.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'help-view := HelpAboutView',
            'if root.help-about-visible: help-view := HelpAboutView'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-LIFECYCLE*"
    }

    It "rejects deriving Help About layout from full window width" {
        $fixture = New-DesktopAuditFixture -Name "help-about-layout-owner"
        $path = Join-Path $fixture "crates\desktop\ui\main.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'out property <string> help-about-layout-mode: help-view.layout-mode;',
            'out property <string> help-about-layout-mode: root.width < 800px ? "narrow" : "wide";'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-VIEW*"
    }

    It "rejects duplicating the Help About section count in MainWindow" {
        $fixture = New-DesktopAuditFixture -Name "help-about-section-owner"
        $path = Join-Path $fixture "crates\desktop\ui\main.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'out property <int> help-about-section-count: help-view.section-count;',
            'out property <int> help-about-section-count: 6;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-BOUND*"
    }

    It "rejects Help About section-count drift" {
        $fixture = New-DesktopAuditFixture -Name "help-about-sections"
        $path = Join-Path $fixture "crates\desktop\ui\views\help-about-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'out property <int> section-count: 6;',
            'out property <int> section-count: 60;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-BOUND*"
    }

    It "rejects a duplicated Help About section instance" {
        $fixture = New-DesktopAuditFixture -Name "help-about-section-duplicate"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\views\help-about-view.slint") `
            -Value 'HelpSectionCard {'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-BOUND*"
    }

    It "rejects removed Help About section instances" {
        $fixture = New-DesktopAuditFixture -Name "help-about-section-removed"
        $path = Join-Path $fixture "crates\desktop\ui\views\help-about-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'HelpSectionCard {',
            'Rectangle {'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-BOUND*"
    }

    It "rejects removing the standard Slint attribution" {
        $fixture = New-DesktopAuditFixture -Name "help-about-attribution"
        $path = Join-Path $fixture "crates\desktop\ui\views\help-about-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'AboutSlint {',
            'Rectangle {'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-ATTRIBUTION*"
    }

    It "rejects clipping the standard Slint attribution" {
        $fixture = New-DesktopAuditFixture -Name "help-about-attribution-height"
        $path = Join-Path $fixture "crates\desktop\ui\views\help-about-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'height: 112px;',
            'height: 72px;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-ATTRIBUTION*"
    }

    It "rejects undersized Help About license reference text" {
        $fixture = New-DesktopAuditFixture -Name "help-about-attribution-font"
        $path = Join-Path $fixture "crates\desktop\ui\views\help-about-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'font-size: 10px;',
            'font-size: 9px;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-ATTRIBUTION*"
    }

    It "rejects an undersized Help About section card" {
        $fixture = New-DesktopAuditFixture -Name "help-about-card-height"
        $path = Join-Path $fixture "crates\desktop\ui\views\help-about-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'property <length> card-height: 232px;',
            'property <length> card-height: 204px;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-VIEW*"
    }

    It "rejects a dynamic Help About version source" {
        $fixture = New-DesktopAuditFixture -Name "help-about-version-source"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'window.set_help_product_version(env!("CARGO_PKG_VERSION").into());',
            'window.set_help_product_version(std::env::var("TOKENMASTER_VERSION").unwrap().into());'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-VERSION*"
    }

    It "rejects duplicate Help About version application" {
        $fixture = New-DesktopAuditFixture -Name "help-about-version-duplicate"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\ui.rs") `
            -Value 'fn duplicate_help_version(window: &MainWindow) { window.set_help_product_version(env!("CARGO_PKG_VERSION").into()); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-VERSION*"
    }

    It "rejects Help About callbacks or control authority" {
        $fixture = New-DesktopAuditFixture -Name "help-about-authority"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\views\help-about-view.slint") `
            -Value 'export component FalseHelpControl { callback activate-benefit(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-AUTHORITY*"
    }

    It "rejects a TokenMaster Help About open URL surface" {
        $fixture = New-DesktopAuditFixture -Name "help-about-open-url"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\views\help-about-view.slint") `
            -Value 'export component FalseHelpLink { in property <string> target; clicked => { Platform.open-url(root.target); } }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-AUTHORITY*"
    }

    It "rejects a Help About list model" {
        $fixture = New-DesktopAuditFixture -Name "help-about-model"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\views\help-about-view.slint") `
            -Value 'export component FalseHelpModel { in property <[string]> rows; }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-BOUND*"
    }

    It "rejects removing the responsive Help About breakpoint" {
        $fixture = New-DesktopAuditFixture -Name "help-about-responsive"
        $path = Join-Path $fixture "crates\desktop\ui\views\help-about-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'out property <bool> narrow: root.width < 800px;',
            'out property <bool> narrow: false;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-VIEW*"
    }

    It "rejects hiding the Help About privacy boundary" {
        $fixture = New-DesktopAuditFixture -Name "help-about-privacy"
        $path = Join-Path $fixture "crates\desktop\ui\views\help-about-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'No prompts, responses, reasoning, commands',
            'Private content may be retained'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-CONTENT*"
    }

    It "rejects misrouting recovery operations to Settings" {
        $fixture = New-DesktopAuditFixture -Name "help-about-operation-owner"
        $path = Join-Path $fixture "crates\desktop\ui\views\help-about-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'Data Health owns backup, verification, restore, rebuild, and recovery truth. Settings owns backup policy and portable configuration.',
            'Settings owns backup, restore, and portable configuration.'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-CONTENT*"
    }

    It "rejects false release readiness in Help About" {
        $fixture = New-DesktopAuditFixture -Name "help-about-false-claim"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\views\help-about-view.slint") `
            -Value '// release accepted'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-CLAIM*"
    }

    It "rejects false automation readiness in Help About" {
        $fixture = New-DesktopAuditFixture -Name "help-about-false-automation"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\views\help-about-view.slint") `
            -Value '// CLI is available'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-CLAIM*"
    }

    It "rejects false provider readiness in Help About" {
        $fixture = New-DesktopAuditFixture -Name "help-about-false-provider"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\views\help-about-view.slint") `
            -Value '// all providers supported'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HELP-ABOUT-CLAIM*"
    }

    It "rejects sessions presentation-bound drift" {
        $fixture = New-DesktopAuditFixture -Name "sessions-bound"
        $path = Join-Path $fixture "crates\desktop\src\sessions.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'pub const MAX_SESSION_ROWS: usize = 64;',
            'pub const MAX_SESSION_ROWS: usize = 640;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-BOUND*"
    }

    It "rejects sessions request-bound drift" {
        $fixture = New-DesktopAuditFixture -Name "sessions-request-bound"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'pub const MAX_SESSION_ROWS: usize = 64;',
            'pub const MAX_SESSION_ROWS: usize = 640;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-REQUEST*"
    }

    It "rejects exact session-detail presentation-bound drift" {
        $fixture = New-DesktopAuditFixture -Name "session-detail-bound"
        $path = Join-Path $fixture "crates\desktop\src\sessions.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'pub const MAX_SESSION_DETAIL_MODEL_ROWS: usize = 32;',
            'pub const MAX_SESSION_DETAIL_MODEL_ROWS: usize = 320;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSION-DETAIL-BOUND*"
    }

    It "rejects replacing the latest-only session-detail work slot with a queue" {
        $fixture = New-DesktopAuditFixture -Name "session-detail-slot"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'pending_selection: Option<PendingDesktopSessionDetail>',
            'pending_selection: Vec<PendingDesktopSessionDetail>'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSION-DETAIL-SLOT*"
    }

    It "rejects hiding a session-detail queue behind a type alias" {
        $fixture = New-DesktopAuditFixture -Name "session-detail-aliased-queue"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        Add-Content -LiteralPath $path `
            -Value 'type DetailQueue = std::collections::VecDeque<DesktopSessionDetailIntent>;'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSION-DETAIL-SLOT*"
    }

    It "accepts an unrelated bounded vector type in the controller" {
        $fixture = New-DesktopAuditFixture -Name "unrelated-bounded-vector"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        Add-Content -LiteralPath $path -Value 'type UnrelatedBoundedData = Vec<u8>;'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Not -Throw
    }

    It "rejects opaque session keys crossing into the UI projection" {
        $fixture = New-DesktopAuditFixture -Name "session-detail-identity"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\sessions.rs") `
            -Value 'struct LeakyDetail { key: UsageSessionKey }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSION-DETAIL-IDENTITY*"
    }

    It "rejects removing the typed session-detail selection callback" {
        $fixture = New-DesktopAuditFixture -Name "session-detail-routing"
        $path = Join-Path $fixture "crates\desktop\ui\main.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'callback select-session(int);',
            'callback select-session-removed(int);'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSION-DETAIL-ROUTING*"
    }

    It "rejects removing tab navigation from session rows" {
        $fixture = New-DesktopAuditFixture -Name "session-detail-tab-navigation"
        $path = Join-Path $fixture "crates\desktop\ui\views\sessions-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'focus-on-tab-navigation: true;',
            'focus-on-tab-navigation: false;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSION-DETAIL-ROUTING*"
    }

    It "rejects a second session-detail model replacement site" {
        $fixture = New-DesktopAuditFixture -Name "session-detail-model"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\ui.rs") `
            -Value 'fn duplicate_detail_model() { window.set_session_detail_breakdown_rows(model(rows)); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSION-DETAIL-MODEL*"
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

    It "rejects Models rebuilding from route selection" {
        $fixture = New-DesktopAuditFixture -Name "route-models-rebuild"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'apply_route_projection(&window, state.projection());',
            "apply_route_projection(&window, state.projection());`r`n            apply_models_projection(&window, state.projection().models());"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-MODELS-REBUILD*"
    }

    It "rejects Projects rebuilding from route selection" {
        $fixture = New-DesktopAuditFixture -Name "route-projects-rebuild"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'apply_route_projection(&window, state.projection());',
            "apply_route_projection(&window, state.projection());`r`n            apply_projects_projection(&window, state.projection().projects());"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PROJECTS-REBUILD*"
    }

    It "rejects sessions model rebuilding from route selection" {
        $fixture = New-DesktopAuditFixture -Name "route-sessions-rebuild"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'apply_route_projection(&window, state.projection());',
            "apply_route_projection(&window, state.projection());`r`n            apply_sessions_projection(&window, state.projection().sessions());"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-REBUILD*"
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
            'latest: Mutex<Option<ReliableStateDelivery>>',
            'latest: Mutex<VecDeque<ReliableStateDelivery>>'
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

    It "rejects widening the command palette query cap" {
        $fixture = New-DesktopAuditFixture -Name "command-palette-cap"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'MAX_COMMAND_PALETTE_QUERY_SCALARS: usize = 64',
            'MAX_COMMAND_PALETTE_QUERY_SCALARS: usize = 65'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-COMMAND-PALETTE-BOUND*"
    }

    It "rejects removing the exact command palette shortcut" {
        $fixture = New-DesktopAuditFixture -Name "command-palette-shortcut"
        $path = Join-Path $fixture "crates\desktop\ui\main.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'keys: @keys(Control + K);',
            'keys: @keys(Control + P);'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-COMMAND-PALETTE-SHORTCUT*"
    }

    It "rejects command palette mutation actions" {
        $fixture = New-DesktopAuditFixture -Name "command-palette-mutation"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\components\command-palette.slint") `
            -Value 'Button { text: "Backup"; }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-COMMAND-PALETTE-ROUTE-ONLY*"
    }

    It "rejects nesting the command palette before the notification layer" {
        $fixture = New-DesktopAuditFixture -Name "command-palette-overlay-order"
        $path = Join-Path $fixture "crates\desktop\ui\main.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'if root.in-app-notification-visible: InAppNotificationPanel {',
            'RoutePalette { }`r`n    if root.in-app-notification-visible: InAppNotificationPanel {'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-COMMAND-PALETTE-OVERLAY*"
    }

    It "accepts the bounded dashboard History Sessions Models Projects Activity Notifications and Help About desktop boundary" {
        $fixture = New-DesktopAuditFixture -Name "library-boundary"

        $receipt = & $Audit -RepositoryRoot $fixture -SourceOnly | ConvertFrom-Json
        $receipt.rust_source_file_count | Should -Be 15
        $receipt.slint_source_file_count | Should -Be 23
        $receipt.command_palette_query_scalar_maximum | Should -Be 64
        $receipt.command_palette_model_count | Should -Be 1
        $receipt.command_palette_shortcut_count | Should -Be 1
        $receipt.command_palette_accessible_default_action_count | Should -Be 1
        $receipt.command_palette_route_only | Should -BeTrue
        $receipt.command_palette_owner_count | Should -Be 0
        $receipt.dashboard_section_count | Should -Be 6
        $receipt.dashboard_model_replacement_count | Should -Be 7
        $receipt.history_day_maximum | Should -Be 30
        $receipt.history_model_replacement_count | Should -Be 1
        $receipt.history_projection_application_count | Should -Be 1
        $receipt.model_row_maximum | Should -Be 64
        $receipt.models_model_replacement_count | Should -Be 1
        $receipt.models_projection_application_count | Should -Be 1
        $receipt.analytics_query_call_count | Should -Be 2
        $receipt.project_row_maximum | Should -Be 32
        $receipt.projects_model_replacement_count | Should -Be 1
        $receipt.projects_projection_application_count | Should -Be 1
        $receipt.git_query_call_count | Should -Be 1
        $receipt.activity_row_maximum | Should -Be 12
        $receipt.activity_model_replacement_count | Should -Be 1
        $receipt.activity_projection_application_count | Should -Be 1
        $receipt.activity_query_call_count | Should -Be 1
        $receipt.activity_polling_surface_count | Should -Be 0
        $receipt.notification_scope_maximum | Should -Be 32
        $receipt.notification_lot_maximum | Should -Be 256
        $receipt.notification_lead_maximum | Should -Be 8
        $receipt.notification_scope_model_replacement_count | Should -Be 1
        $receipt.notification_lot_model_replacement_count | Should -Be 1
        $receipt.notifications_projection_application_count | Should -Be 1
        $receipt.benefit_query_call_count | Should -Be 1
        $receipt.notifications_delivery_authority_count | Should -Be 0
        $receipt.notifications_owner_control_count | Should -Be 0
        $receipt.notifications_polling_surface_count | Should -Be 0
        $receipt.help_about_section_count | Should -Be 6
        $receipt.help_about_version_setter_count | Should -Be 1
        $receipt.help_about_slint_attribution_count | Should -Be 1
        $receipt.help_about_model_count | Should -Be 0
        $receipt.help_about_authority_count | Should -Be 0
        $receipt.help_about_polling_surface_count | Should -Be 0
        $receipt.session_row_maximum | Should -Be 64
        $receipt.session_detail_model_row_maximum | Should -Be 32
        $receipt.session_detail_project_row_maximum | Should -Be 32
        $receipt.sessions_model_replacement_count | Should -Be 1
        $receipt.session_detail_model_replacement_count | Should -Be 1
        $receipt.sessions_projection_application_count | Should -Be 1
        $receipt.restore_point_maximum | Should -Be 15
        $receipt.restore_model_replacement_count | Should -Be 1
        $receipt.secret_model_count | Should -Be 0
        $receipt.event_loop_schedule_site_count | Should -Be 2
        $receipt.bridge_event_loop_schedule_site_count | Should -Be 1
        $receipt.reliable_event_loop_schedule_site_count | Should -Be 1
        $receipt.in_app_notification_row_maximum | Should -Be 256
        $receipt.in_app_notification_model_count | Should -Be 1
        $receipt.in_app_notification_presented_after_apply_count | Should -Be 1
        $receipt.in_app_notification_ready_before_receipt_count | Should -Be 1
        $receipt.in_app_notification_accessible_label_count | Should -Be 1
        $receipt.in_app_notification_epoch_guard_count | Should -Be 1
    }

    It "rejects widening the transient in-app notification batch" {
        $fixture = New-DesktopAuditFixture -Name "in-app-cap-drift"
        $path = Join-Path $fixture "crates\desktop\src\in_app_notification.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'MAX_DESKTOP_IN_APP_NOTIFICATIONS: usize = 256',
            'MAX_DESKTOP_IN_APP_NOTIFICATIONS: usize = 257'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-IN-APP-BOUND*"
    }

    It "rejects a presentation receipt that no longer follows visible application" {
        $fixture = New-DesktopAuditFixture -Name "in-app-receipt-order-drift"
        $path = Join-Path $fixture "crates\desktop\src\in_app_notification.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'receipt.presented();',
            'receipt.failed();'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-IN-APP-RECEIPT*"
    }

    It "rejects keeping the bridge busy while invoking the receipt" {
        $fixture = New-DesktopAuditFixture -Name "in-app-ready-order-drift"
        $path = Join-Path $fixture "crates\desktop\src\in_app_notification.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'self.scheduled.store(false, Ordering::Release);',
            'let _ = self.scheduled.load(Ordering::Acquire);'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-IN-APP-RECEIPT*"
    }

    It "rejects omitting the visible benefit label from accessibility text" {
        $fixture = New-DesktopAuditFixture -Name "in-app-accessible-label-drift"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '{benefit_label}. {kind_label}, quantity {quantity_label}',
            '{kind_label}, quantity {quantity_label}'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-IN-APP-ACCESSIBILITY*"
    }

    It "rejects removing the checked notification epoch invalidation" {
        $fixture = New-DesktopAuditFixture -Name "in-app-epoch-drift"
        $path = Join-Path $fixture "crates\desktop\src\in_app_notification.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'self.epochs.deactivate(self.epoch);',
            'let _ = self.epoch;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-IN-APP-EPOCH*"
    }

    It "rejects a second transient notification model" {
        $fixture = New-DesktopAuditFixture -Name "in-app-second-model"
        $path = Join-Path $fixture "crates\desktop\ui\main.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'in property <[InAppNotificationRow]> in-app-notification-rows;',
            "in property <[InAppNotificationRow]> in-app-notification-rows;`r`n    in property <[InAppNotificationRow]> in-app-notification-queue;"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-IN-APP-MODEL*"
    }

    It "rejects private delivery identity in the presentation value" {
        $fixture = New-DesktopAuditFixture -Name "in-app-private-identity"
        $path = Join-Path $fixture "crates\desktop\src\in_app_notification.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'pub struct DesktopInAppNotification {',
            "pub struct DesktopInAppNotification {`r`n    delivery_id: String,"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-IN-APP-IDENTITY*"
    }

    It "rejects timers or automatic dismissal in the transient panel" {
        $fixture = New-DesktopAuditFixture -Name "in-app-timer"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\components\in-app-notification-panel.slint") `
            -Value 'component FalseAutoHide { Timer { interval: 1s; } }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-UI-POLLING*"
    }

    It "rejects widening the fixed desktop reminder lead cap" {
        $fixture = New-DesktopAuditFixture -Name "reminder-lead-cap-drift"
        $path = Join-Path $fixture "crates\desktop\src\reliable_state.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'MAX_DESKTOP_REMINDER_LEADS: usize = 8',
            'MAX_DESKTOP_REMINDER_LEADS: usize = 9'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-REMINDER-BOUND*"
    }

    It "rejects a sixth reminder preset" {
        $fixture = New-DesktopAuditFixture -Name "reminder-preset-count-drift"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\main.slint") `
            -Value 'in-out property <bool> reminder-preset-extra: false;'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-REMINDER-BOUND*"
    }

    It "rejects widening the fixed reminder draft rows" {
        $fixture = New-DesktopAuditFixture -Name "reminder-row-cap-drift"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [regex]::Replace(
            [System.IO.File]::ReadAllText($path),
            'rows\.resize\(\s*8,',
            'rows.resize(9,',
            1
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-REMINDER-MODEL*"
    }

    It "rejects unchecked custom reminder conversion" {
        $fixture = New-DesktopAuditFixture -Name "reminder-unchecked-conversion"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'let lead = value.checked_mul(unit)?;',
            'let lead = value * unit;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-REMINDER-CONVERSION*"
    }

    It "rejects overwriting a dirty reminder draft on publication" {
        $fixture = New-DesktopAuditFixture -Name "reminder-dirty-draft-drift"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'if !window.get_reminder_dirty() {',
            'if true {'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-REMINDER-DRAFT*"
    }

    It "rejects acknowledging Pending before the visible projection is applied" {
        $fixture = New-DesktopAuditFixture -Name "reminder-visible-pending-order"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'acknowledgement.send(if delivered',
            'acknowledgement.send(if true'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-REMINDER-VISIBLE-PENDING*"
    }

    It "rejects widening the visible Pending acknowledgement timeout" {
        $fixture = New-DesktopAuditFixture -Name "reminder-visible-pending-timeout"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'VISIBLE_REMINDER_PUBLICATION_TIMEOUT: Duration = Duration::from_secs(5)',
            'VISIBLE_REMINDER_PUBLICATION_TIMEOUT: Duration = Duration::from_secs(50)'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-REMINDER-VISIBLE-PENDING*"
    }

    It "rejects removing a custom reminder accessibility label" {
        $fixture = New-DesktopAuditFixture -Name "reminder-accessibility-drift"
        $path = Join-Path $fixture "crates\desktop\ui\views\settings-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'accessible-label: "Custom reminder lead unit row " + (index + 1);',
            'accessible-label: "Custom unit";'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-REMINDER-ACCESSIBILITY*"
    }

    It "rejects replacing the intrinsic reminder ScrollView card" {
        $fixture = New-DesktopAuditFixture -Name "reminder-scroll-drift"
        $path = Join-Path $fixture "crates\desktop\ui\views\settings-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'settings-scroll := ScrollView {',
            'settings-scroll := VerticalLayout {'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-REMINDER-LAYOUT*"
    }
}
