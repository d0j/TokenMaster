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

    It "accepts the bounded dashboard History Sessions Models Projects and Activity desktop boundary" {
        $fixture = New-DesktopAuditFixture -Name "library-boundary"

        $receipt = & $Audit -RepositoryRoot $fixture -SourceOnly | ConvertFrom-Json
        $receipt.rust_source_file_count | Should -Be 13
        $receipt.slint_source_file_count | Should -Be 19
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
    }
}
