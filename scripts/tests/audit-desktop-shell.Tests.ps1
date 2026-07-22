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
            $appSource = Join-Path $fixture "crates\app\src"
            New-Item -ItemType Directory -Path $appSource -Force | Out-Null
            foreach ($relative in @("operation.rs", "operation_tests.rs", "state.rs")) {
                Copy-Item -LiteralPath (Join-Path $RepositoryRoot "crates\app\src\$relative") `
                    -Destination $appSource
            }
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

    It "ignores test-only seeded history fixtures when checking production authority" {
        $fixture = New-DesktopAuditFixture -Name "history-range-test-only-seed"

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Not -Throw
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
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $text = $original.Replace(
            'let worker = RefreshWorker::spawn_notified(',
            'let _extra_worker = RefreshWorker::spawn(fake_clock, fake_execute);`r`n        let worker = RefreshWorker::spawn_notified('
        )
        $text | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-CONTROLLER-WORKER*"
    }

    It "rejects removing the controller completion notifier" {
        $fixture = New-DesktopAuditFixture -Name "terminal-navigation-worker-notifier"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $text = $original.Replace(
            'RefreshWorker::spawn_notified(',
            'RefreshWorker::spawn_without_notifier('
        )
        $text | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-TERMINAL-RECOVERY*"
    }

    It "rejects a comment-only controller completion notifier anchor" {
        $fixture = New-DesktopAuditFixture -Name "terminal-navigation-worker-comment-anchor"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $text = $original.Replace(
            'RefreshWorker::spawn_notified(',
            "RefreshWorker::spawn_without_notifier(`r`n            // RefreshWorker::spawn_notified("
        )
        $text | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-TERMINAL-RECOVERY*"
    }

    It "rejects clearing terminal navigation outside the idempotent completion handler" {
        $fixture = New-DesktopAuditFixture -Name "terminal-navigation-early-clear"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $text = [regex]::Replace(
            $original,
            'if !navigation_is_current\(reducer, context, permit\.id\(\)\.get\(\), intent\) \{\r?\n\s*return RefreshOutcome::Completed;',
            "if !navigation_is_current(reducer, context, permit.id().get(), intent) {`r`n        invalidate_navigation(&mut lock_work(context.work)?);`r`n        return RefreshOutcome::Completed;",
            1
        )
        $text | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-TERMINAL-RECOVERY*"
    }

    It "rejects notifying refresh supersession while holding the work lock" {
        $fixture = New-DesktopAuditFixture -Name "terminal-navigation-lock-order"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $text = [regex]::Replace(
            $original,
            'drop\(work\);\r?\n\s*notify_terminal_navigation\(',
            'notify_terminal_navigation(',
            1
        )
        $text | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-TERMINAL-RECOVERY*"
    }

    It "rejects an unbounded terminal navigation bridge slot" {
        $fixture = New-DesktopAuditFixture -Name "terminal-navigation-unbounded-slot"
        $path = Join-Path $fixture "crates\desktop\src\bridge.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $text = $original.Replace(
            'terminal_intent: std::sync::Mutex<Option<DesktopSessionPageIntent>>',
            'terminal_intent: std::sync::Mutex<Vec<DesktopSessionPageIntent>>'
        )
        $text | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-TERMINAL-RECOVERY*"
    }

    It "rejects bypassing terminal navigation event-loop scheduling" {
        $fixture = New-DesktopAuditFixture -Name "terminal-navigation-schedule-bypass"
        $path = Join-Path $fixture "crates\desktop\src\bridge.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $text = $original.Replace('self.request();', 'self.skip_request();')
        $text | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-TERMINAL-RECOVERY*"
    }

    It "rejects scheduling terminal navigation before releasing the pending slot" {
        $fixture = New-DesktopAuditFixture -Name "terminal-navigation-pending-lock-order"
        $path = Join-Path $fixture "crates\desktop\src\bridge.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $text = [regex]::Replace(
            $original,
            'drop\(pending\);\r?\n\s*self\.request\(\);',
            "self.request();`r`n        drop(pending);",
            1
        )
        $text | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-TERMINAL-RECOVERY*"
    }

    It "rejects an unreachable terminal navigation schedule" {
        $fixture = New-DesktopAuditFixture -Name "terminal-navigation-unreachable-schedule"
        $path = Join-Path $fixture "crates\desktop\src\bridge.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $text = [regex]::Replace(
            $original,
            'drop\(pending\);\r?\n\s*self\.request\(\);',
            "if false {`r`n            drop(pending);`r`n            self.request();`r`n        }",
            1
        )
        $text | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-TERMINAL-RECOVERY*"
    }

    It "rejects bypassing the weak terminal navigation notifier route" {
        $fixture = New-DesktopAuditFixture -Name "terminal-navigation-notifier-bypass"
        $path = Join-Path $fixture "crates\desktop\src\bridge.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $text = $original.Replace(
            'inner.request_terminal(intent);',
            'inner.skip_terminal(intent);'
        )
        $text | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-TERMINAL-RECOVERY*"
    }

    It "rejects an unreachable weak terminal navigation notifier route" {
        $fixture = New-DesktopAuditFixture -Name "terminal-navigation-unreachable-notifier"
        $path = Join-Path $fixture "crates\desktop\src\bridge.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $text = $original.Replace(
            'inner.request_terminal(intent);',
            'if false { inner.request_terminal(intent); }'
        )
        $text | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-TERMINAL-RECOVERY*"
    }

    It "reports executable terminal navigation recovery counts" {
        $fixture = New-DesktopAuditFixture -Name "terminal-navigation-receipt"

        $receipt = (& $Audit -RepositoryRoot $fixture -SourceOnly) | ConvertFrom-Json
        $receipt.notified_controller_worker_count | Should -Be 1
        $receipt.terminal_navigation_slot_count | Should -Be 1
        $receipt.terminal_navigation_route_count | Should -Be 1
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

    It "rejects comment-only history bound anchors" {
        $fixture = New-DesktopAuditFixture -Name "history-bound-comment-anchor"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('pub const MAX_HISTORY_DAYS: usize = 30;', "// pub const MAX_HISTORY_DAYS: usize = 30;`r`npub const MAX_HISTORY_DAYS: usize = 31;")
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-BOUND*"
    }

    It "rejects literal-only history bound anchors" {
        $fixture = New-DesktopAuditFixture -Name "history-bound-literal-anchor"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('pub const MAX_HISTORY_DAYS: usize = 30;', "const _DECOY: &str = ""pub const MAX_HISTORY_DAYS: usize = 30;"";`r`npub const MAX_HISTORY_DAYS: usize = 31;")
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-BOUND*"
    }

    It "rejects a real thirty-one-row history projection" {
        $fixture = New-DesktopAuditFixture -Name "history-bound-real-thirty-one"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('.take(MAX_HISTORY_DAYS)', '.take(31)')
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-BOUND*"
    }

    It "rejects a differently named history bound constant" {
        $fixture = New-DesktopAuditFixture -Name "history-bound-renamed-constant"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('MAX_HISTORY_DAYS', 'MAX_HISTORY_ROWS')
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-BOUND*"
    }

    It "rejects feature-cfg duplicate raw History bound symbols" {
        $fixture = New-DesktopAuditFixture -Name "history-bound-feature-cfg-duplicates"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('pub const MAX_HISTORY_DAYS: usize = 30;', "#[cfg(feature = ""decoy"")]`r`npub const MAX_HISTORY_DAYS: usize = 30;`r`n#[cfg(feature = ""decoy"")]`r`npub(crate) fn from_snapshot_with_range() {}`r`npub const MAX_HISTORY_DAYS: usize = 31;")
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-UNIQUE-DEFINITION*"
    }

    It "rejects a constant-only feature-cfg History bound decoy" {
        $fixture = New-DesktopAuditFixture -Name "history-bound-feature-cfg-constant-only-decoy"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('pub const MAX_HISTORY_DAYS: usize = 30;', "#[cfg(feature = ""decoy"")]`r`npub const MAX_HISTORY_DAYS: usize = 30;`r`npub const MAX_HISTORY_DAYS: usize = 31;")
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-UNIQUE-DEFINITION*"
    }

    It "rejects a cfg-decorated sole History bound constant" {
        $fixture = New-DesktopAuditFixture -Name "history-bound-cfg-decorated-sole-constant"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('pub const MAX_HISTORY_DAYS: usize = 30;', "#[cfg(debug_assertions)]`r`npub const MAX_HISTORY_DAYS: usize = 30;")
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-CFG*"
    }

    It "rejects a multiline cfg-decorated sole History bound constant" {
        $fixture = New-DesktopAuditFixture -Name "history-bound-multiline-cfg-constant"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('pub const MAX_HISTORY_DAYS: usize = 30;', "#[cfg(`r`n    debug_assertions`r`n)]`r`npub const MAX_HISTORY_DAYS: usize = 30;")
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-CFG*"
    }

    It "rejects a multiline cfg-attr-decorated sole History bound constant" {
        $fixture = New-DesktopAuditFixture -Name "history-bound-multiline-cfg-attr-constant"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('pub const MAX_HISTORY_DAYS: usize = 30;', "#[cfg_attr(`r`n    debug_assertions,`r`n    allow(dead_code)`r`n)]`r`npub const MAX_HISTORY_DAYS: usize = 30;")
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-CFG*"
    }

    It "rejects a spaced hash cfg-decorated sole History bound constant" {
        $fixture = New-DesktopAuditFixture -Name "history-bound-spaced-hash-cfg-constant"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('pub const MAX_HISTORY_DAYS: usize = 30;', "# [cfg(debug_assertions)]`r`npub const MAX_HISTORY_DAYS: usize = 30;")
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-CFG*"
    }

    It "rejects a comment-separated hash cfg-decorated sole History bound constant" {
        $fixture = New-DesktopAuditFixture -Name "history-bound-comment-separated-cfg-constant"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('pub const MAX_HISTORY_DAYS: usize = 30;', "# /*attribute separator*/ [cfg(debug_assertions)]`r`npub const MAX_HISTORY_DAYS: usize = 30;")
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-CFG*"
    }

    It "rejects a raw cfg-decorated sole History bound constant" {
        $fixture = New-DesktopAuditFixture -Name "history-bound-raw-cfg-constant"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('pub const MAX_HISTORY_DAYS: usize = 30;', "#[r#cfg(debug_assertions)]`r`npub const MAX_HISTORY_DAYS: usize = 30;")
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-CFG*"
    }

    It "rejects a raw cfg-attr-decorated sole History bound constant" {
        $fixture = New-DesktopAuditFixture -Name "history-bound-raw-cfg-attr-constant"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('pub const MAX_HISTORY_DAYS: usize = 30;', "#[r#cfg_attr(debug_assertions, allow(dead_code))]`r`npub const MAX_HISTORY_DAYS: usize = 30;")
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-CFG*"
    }

    It "rejects a raw spaced cfg branch in the History projection body" {
        $fixture = New-DesktopAuditFixture -Name "history-bound-raw-spaced-cfg-branch"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('.take(MAX_HISTORY_DAYS)', 'if r#cfg ! (debug_assertions) { points.take(MAX_HISTORY_DAYS) } else { points.take(31) }')
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-CFG*"
    }

    It "rejects a brace cfg branch in the History projection body" {
        $fixture = New-DesktopAuditFixture -Name "history-bound-brace-cfg-branch"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('.take(MAX_HISTORY_DAYS)', 'if cfg! { points.take(MAX_HISTORY_DAYS) } else { points.take(31) }')
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-CFG*"
    }

    It "rejects a bracket cfg branch in the History projection body" {
        $fixture = New-DesktopAuditFixture -Name "history-bound-bracket-cfg-branch"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('.take(MAX_HISTORY_DAYS)', 'if cfg![debug_assertions] { points.take(MAX_HISTORY_DAYS) } else { points.take(31) }')
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-CFG*"
    }

    It "rejects an indirect production cfg helper outside the History projection" {
        $fixture = New-DesktopAuditFixture -Name "history-indirect-production-cfg-helper"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('pub const MAX_HISTORY_DAYS: usize = 30;', "const HISTORY_RELEASE_BOUND: usize = if cfg! { 30 } else { 31 };`r`npub const MAX_HISTORY_DAYS: usize = 30;")
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-CFG*"
    }

    It "rejects an all-source dashboard cfg helper" {
        $fixture = New-DesktopAuditFixture -Name "history-global-dashboard-cfg-helper"
        $path = Join-Path $fixture "crates\desktop\src\dashboard.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $mutated = $original + "`r`nconst DASHBOARD_CFG_HELPER: usize = if cfg! { 30 } else { 31 };`r`n"
        $mutated | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $mutated)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-CFG*"
    }

    It "rejects a production cfg helper after a test module" {
        $fixture = New-DesktopAuditFixture -Name "history-post-test-production-cfg-helper"
        $path = Join-Path $fixture "crates\desktop\src\presentation.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $mutated = $original + "`r`nconst POST_TEST_CFG_HELPER: usize = if cfg! { 30 } else { 31 };`r`n"
        $mutated | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $mutated)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-CFG*"
    }

    It "allows a prepended top-level test-only cfg module" {
        $fixture = New-DesktopAuditFixture -Name "history-prepended-test-only-cfg-module"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $mutated = "#[cfg(test)]`r`nmod cfg_fixture { const _: bool = cfg! { true }; }`r`n" + $original
        $mutated | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $mutated)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Not -Throw
    }

    It "allows a test-only associated cfg method" {
        $fixture = New-DesktopAuditFixture -Name "history-test-only-associated-cfg-method"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $mutated = $original.Replace('impl DesktopHistoryProjection {', "impl DesktopHistoryProjection {`r`n    #[cfg(test)]`r`n    fn cfg_fixture() {}")
        $mutated | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $mutated)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Not -Throw
    }

    It "rejects a third native tray platform cfg item" {
        $fixture = New-DesktopAuditFixture -Name "history-native-tray-third-platform-cfg"
        $path = Join-Path $fixture "crates\desktop\src\native_tray.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $mutated = $original + "`r`n#[cfg(target_os = ""windows"")]`r`nmod extra_platform_cfg {}`r`n"
        $mutated | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $mutated)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-CFG*"
    }

    It "rejects native tray platform cfg literal drift" {
        $fixture = New-DesktopAuditFixture -Name "history-native-tray-platform-literal-drift"
        $path = Join-Path $fixture "crates\desktop\src\native_tray.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $mutated = $original.Replace('target_os = "windows"', 'target_os = "linux"')
        $mutated | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $mutated)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-CFG*"
    }

    It "allows a delimiter-aware test-only cfg method signature" {
        $fixture = New-DesktopAuditFixture -Name "history-test-only-cfg-array-signature"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $mutated = $original.Replace('impl DesktopHistoryProjection {', "impl DesktopHistoryProjection {`r`n    #[cfg(test)]`r`n    fn cfg_array_fixture(_: [u8; 1]) { const _: bool = cfg! { true }; }")
        $mutated | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $mutated)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Not -Throw
    }

    It "handles an empty extracted History body before its semantic gate" {
        $fixture = New-DesktopAuditFixture -Name "history-empty-extracted-body"
        $path = Join-Path $fixture "crates\desktop\src\presentation.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('self.reject_history_range(intent);', '')
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-TERMINAL*"
    }

    It "rejects an arbitrary history range count" {
        $fixture = New-DesktopAuditFixture -Name "history-range-arbitrary-count"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'Self::Recent7Days => 7,',
            'Self::Recent7Days => days,'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-RANGE-PRESETS*"
    }

    It "rejects a fourth history range preset" {
        $fixture = New-DesktopAuditFixture -Name "history-range-fourth-preset"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '    Recent30Days,',
            "    Recent30Days,`r`n    Recent90Days,"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-RANGE-PRESETS*"
    }

    It "rejects a feature-cfg duplicate audited History definition" {
        $fixture = New-DesktopAuditFixture -Name "history-range-feature-cfg-duplicate-definition"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $replacement = @'
#[cfg(feature = "decoy")]
pub enum DesktopHistoryRangePreset {
    Decoy,
}

#[cfg(test)]
'@
        $text = [regex]::Replace($original, '(?m)^#\[cfg\(test\)\](?=\r?$)', $replacement, 1)
        $text | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-RANGE-UNIQUE-DEFINITION*"
    }

    It "rejects a feature-cfg duplicate History generation fence definition" {
        $fixture = New-DesktopAuditFixture -Name "history-range-feature-cfg-duplicate-generation-fence"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $replacement = @'
#[cfg(feature = "decoy")]
fn history_range_generation_is_current() -> bool {
    false
}

#[cfg(test)]
'@
        $text = [regex]::Replace($original, '(?m)^#\[cfg\(test\)\](?=\r?$)', $replacement, 1)
        $text | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-RANGE-UNIQUE-DEFINITION*"
    }

    It "rejects feature-cfg duplicate commit rebind and execute History definitions" {
        $fixture = New-DesktopAuditFixture -Name "history-range-feature-cfg-controller-definition-decoys"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $replacement = @'
#[cfg(feature = "decoy")]
fn commit_history_range() {}
#[cfg(feature = "decoy")]
fn rebind_history_range_after_refresh() {}
#[cfg(feature = "decoy")]
fn execute_history_range() {}

#[cfg(test)]
'@
        $text = [regex]::Replace($original, '(?m)^#\[cfg\(test\)\](?=\r?$)', $replacement, 1)
        $text | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-RANGE-UNIQUE-DEFINITION*"
    }

    It "rejects a feature-cfg duplicate refresh executor definition" {
        $fixture = New-DesktopAuditFixture -Name "history-range-feature-cfg-refresh-executor"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $text = [regex]::Replace($original, '(?m)^#\[cfg\(test\)\](?=\r?$)', "#[cfg(feature = ""decoy"")]`r`nfn execute_refresh() {}`r`n`r`n#[cfg(test)]", 1)
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-UNIQUE-DEFINITION*"
    }

    It "rejects a feature-cfg duplicate presentation History definition" {
        $fixture = New-DesktopAuditFixture -Name "history-range-feature-cfg-presentation-definition-decoy"
        $path = Join-Path $fixture "crates\desktop\src\presentation.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $replacement = @'
#[cfg(feature = "decoy")]
fn complete_history_range_terminal() {}

#[cfg(test)]
'@
        $text = [regex]::Replace($original, '(?m)^#\[cfg\(test\)\](?=\r?$)', $replacement, 1)
        $text | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-RANGE-UNIQUE-DEFINITION*"
    }

    It "rejects retaining history range work in a vector" {
        $fixture = New-DesktopAuditFixture -Name "history-range-vector-state"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'pending_history_range: Option<PendingDesktopHistoryRange>,',
            'pending_history_range: Vec<PendingDesktopHistoryRange>,'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-RANGE-STATE*"
    }

    It "rejects a History range backlog alias in the whole work-state schema" {
        $fixture = New-DesktopAuditFixture -Name "history-range-backlog-alias"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('pending_history_range: Option<PendingDesktopHistoryRange>,', "type RangeBacklog = Vec<PendingDesktopHistoryRange>;`r`n    pending_history_range: Option<PendingDesktopHistoryRange>,`r`n    range_backlog: RangeBacklog,")
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-STATE*"
    }

    It "rejects a feature-cfg DesktopWorkState decoy before an expanded real schema" {
        $fixture = New-DesktopAuditFixture -Name "history-range-feature-cfg-work-state-decoy"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('struct DesktopWorkState {', "#[cfg(feature = ""decoy"")]`r`nstruct DesktopWorkState {}`r`nstruct DesktopWorkState {").Replace('pending_history_range: Option<PendingDesktopHistoryRange>,', "pending_history_range: Option<PendingDesktopHistoryRange>,`r`n    range_backlog: Vec<PendingDesktopHistoryRange>,")
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-UNIQUE-DEFINITION*"
    }

    It "rejects a renamed History range backlog by exact state type" {
        $fixture = New-DesktopAuditFixture -Name "history-range-renamed-backlog-type"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'pending_history_range: Option<PendingDesktopHistoryRange>,',
            "pending_history_range: Option<PendingDesktopHistoryRange>,`r`n    range_backlog: Vec<PendingDesktopHistoryRange>,"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-RANGE-STATE*"
    }

    It "rejects removing the exact history range generation fence" {
        $fixture = New-DesktopAuditFixture -Name "history-range-generation-fence"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '                && history_range_generation_is_current(',
            '                && generation_fence_removed('
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-RANGE-FENCES*"
    }

    It "rejects resetting the selected history range after a refresh" {
        $fixture = New-DesktopAuditFixture -Name "history-range-refresh-reset"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'current.rebound_product_generation = Some(product_generation);',
            "current.rebound_product_generation = Some(product_generation);`r`n        state.published_history_preset = DesktopHistoryRangePreset::Recent30Days;"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-RANGE-REFRESH*"
    }

    It "rejects a stale history terminal rollback" {
        $fixture = New-DesktopAuditFixture -Name "history-range-stale-terminal"
        $path = Join-Path $fixture "crates\desktop\src\presentation.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'if self.active_history_range == Some(intent) {',
            'if self.active_history_range.is_some() {'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-RANGE-TERMINAL*"
    }

    It "rejects query authority in the history range callback" {
        $fixture = New-DesktopAuditFixture -Name "history-range-callback-query"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'if sink.submit(intent) == DesktopHistoryRangeIntentAdmission::Rejected {',
            'source.usage_analytics(request); if sink.submit(intent) == DesktopHistoryRangeIntentAdmission::Rejected {'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-UI-QUERY*"
    }

    It "rejects a duplicate history analytics authority" {
        $fixture = New-DesktopAuditFixture -Name "history-range-extra-query"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'let result = source',
            'let _duplicate = source.usage_analytics(request.clone()); let result = source'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-MODELS-REQUEST*"
    }

    It "rejects a disabled history fence decoy" {
        $fixture = New-DesktopAuditFixture -Name "history-range-disabled-fence-decoy"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'current.intent == intent',
            'current.intent != intent && if false { current.intent == intent; false } else { true }'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-RANGE-FENCES*"
    }

    It "rejects a differently named history cache field" {
        $fixture = New-DesktopAuditFixture -Name "history-range-hidden-cache"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'pending_history_range: Option<PendingDesktopHistoryRange>,',
            "pending_history_range: Option<PendingDesktopHistoryRange>,`r`n    history_result_cache: Vec<PendingDesktopHistoryRange>,"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-RANGE-STATE*"
    }

    It "rejects a fourth analytics owner outside refresh and history range execution" {
        $fixture = New-DesktopAuditFixture -Name "history-range-fourth-query-owner"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        Add-Content -LiteralPath $path -Value 'fn history_query_decoy<S: DesktopQuerySource>(source: &mut S, request: UsageAnalyticsRequest) { let _ = source.usage_analytics(request); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-MODELS-REQUEST*"
    }

    It "rejects publishing a nonaccepted history range outcome" {
        $fixture = New-DesktopAuditFixture -Name "history-range-nonaccepted-publication"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'ProductPublishOutcome::RejectedOlder',
            'ProductPublishOutcome::Accepted'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-RANGE-ACCEPTANCE*"
    }

    It "rejects retaining thirty-one history rows" {
        $fixture = New-DesktopAuditFixture -Name "history-range-thirty-one-rows"
        $path = Join-Path $fixture "crates\desktop\src\history.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '.take(MAX_HISTORY_DAYS)',
            '.take(31)'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-BOUND*"
    }

    It "rejects a comment decoy for the current history intent fence" {
        $fixture = New-DesktopAuditFixture -Name "history-range-comment-decoy"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('current.intent == intent', 'current.intent != intent // current.intent == intent')
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-FENCES*"
    }

    It "rejects a literal decoy for the current history intent fence" {
        $fixture = New-DesktopAuditFixture -Name "history-range-literal-decoy"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('current.intent == intent', 'current.intent != intent; let _ = "current.intent == intent";')
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-FENCES*"
    }

    It "rejects cfg-any history fence decoy" {
        $fixture = New-DesktopAuditFixture -Name "history-range-cfg-any-decoy"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('current.intent == intent', 'current.intent != intent && if cfg!(any()) { current.intent == intent } else { true }')
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-CFG*"
    }

    It "rejects an inline cfg anchor in an audited History body" {
        $fixture = New-DesktopAuditFixture -Name "history-range-inline-cfg-anchor"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'current.intent == intent',
            'current.intent == intent && cfg!(feature = "decoy")'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-RANGE-CFG*"
    }

    It "rejects inverted epoch fence" {
        $fixture = New-DesktopAuditFixture -Name "history-range-epoch-inversion"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('!= intent.snapshot_epoch().get()', '== intent.snapshot_epoch().get()')
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-FENCES*"
    }

    It "rejects inverted attempt fence" {
        $fixture = New-DesktopAuditFixture -Name "history-range-attempt-inversion"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('current.attempt == attempt', 'current.attempt != attempt')
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-FENCES*"
    }

    It "rejects inverted product generation fence" {
        $fixture = New-DesktopAuditFixture -Name "history-range-product-inversion"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('== product_generation', '!= product_generation')
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-FENCES*"
    }

    It "rejects an unused History intent equality anchor outside the validity predicate" {
        $fixture = New-DesktopAuditFixture -Name "history-range-unused-intent-anchor"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'current.intent == intent',
            'true && { let _ = current.intent == intent; true }'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-RANGE-FENCES*"
    }

    It "rejects an unused History attempt equality anchor outside the validity predicate" {
        $fixture = New-DesktopAuditFixture -Name "history-range-unused-attempt-anchor"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'current.attempt == attempt',
            'true && { let _ = current.attempt == attempt; true }'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-RANGE-FENCES*"
    }

    It "rejects an unused History product equality anchor outside the validity predicate" {
        $fixture = New-DesktopAuditFixture -Name "history-range-unused-product-anchor"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '== product_generation',
            '!= product_generation && { let _ = current.intent.product_generation() == product_generation; true }'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-RANGE-FENCES*"
    }

    It "rejects bypassing the History publication action helper in commit" {
        $fixture = New-DesktopAuditFixture -Name "history-range-commit-helper-bypass"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'match history_range_publication_action(outcome, successful) {',
            'let _ = history_range_publication_action(outcome, successful); match HistoryRangePublicationAction::PublishWithoutPresetAdvance {'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-RANGE-ACCEPTANCE*"
    }

    It "rejects duplicate History bridge slot without displacing Sessions" {
        $fixture = New-DesktopAuditFixture -Name "history-range-duplicate-bridge-slot"
        $path = Join-Path $fixture "crates\desktop\src\bridge.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('history_terminal_intent: std::sync::Mutex<Option<DesktopHistoryRangeIntent>>,', "history_terminal_intent: std::sync::Mutex<Option<DesktopHistoryRangeIntent>>,`r`n    extra_history_terminal_intent: std::sync::Mutex<Option<DesktopHistoryRangeIntent>>,")
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-BRIDGE-SLOT*"
    }

    It "rejects a renamed duplicate History bridge slot" {
        $fixture = New-DesktopAuditFixture -Name "history-range-renamed-duplicate-bridge-slot"
        $path = Join-Path $fixture "crates\desktop\src\bridge.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('history_terminal_intent: std::sync::Mutex<Option<DesktopHistoryRangeIntent>>,', "history_terminal_intent: std::sync::Mutex<Option<DesktopHistoryRangeIntent>>,`r`n    latest_terminal: std::sync::Mutex<Option<DesktopHistoryRangeIntent>>,")
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-BRIDGE-SLOT*"
    }

    It "rejects duplicate History controller slot without displacing Sessions" {
        $fixture = New-DesktopAuditFixture -Name "history-range-duplicate-controller-slot"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [regex]::Replace([System.IO.File]::ReadAllText($path), '(?s)(pub struct DesktopController\s*\{.*?terminal_history_range_notifier: TerminalHistoryRangeNotifier,)', '${1}' + "`r`n    extra_history_terminal_notifier: TerminalHistoryRangeNotifier,", 1)
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-CONTROLLER-SLOT*"
    }

    It "rejects a renamed duplicate History controller slot" {
        $fixture = New-DesktopAuditFixture -Name "history-range-renamed-duplicate-controller-slot"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [regex]::Replace([System.IO.File]::ReadAllText($path), '(?s)(pub struct DesktopController\s*\{.*?terminal_history_range_notifier: TerminalHistoryRangeNotifier,)', '${1}' + "`r`n    current_terminal: TerminalHistoryRangeNotifier,", 1)
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } | Should -Throw "*TM-DESKTOP-HISTORY-RANGE-CONTROLLER-SLOT*"
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
            'apply_route_projection(window, projection);',
            "apply_route_projection(window, projection);`r`n    apply_activity_route_projection(window, projection.activity());"
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
            'apply_route_projection(window, projection);',
            "apply_route_projection(window, projection);`r`n    apply_notifications_projection(window, projection.notifications());"
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

    It "rejects a second production Sessions projection caller" {
        $fixture = New-DesktopAuditFixture -Name "sessions-page-second-caller"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '#[cfg(test)]',
            "fn apply_accepted_sessions_page_replacement(window: &MainWindow, sessions: &DesktopSessionsProjection) { apply_sessions_projection /* replace */ (window, sessions); }`r`n`r`n#[cfg(test)]"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-REBUILD*"
    }

    It "reports one Sessions projection caller with legal block-comment spacing" {
        $fixture = New-DesktopAuditFixture -Name "sessions-page-commented-caller-receipt"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $text = $original.Replace(
            'apply_sessions_projection(window, projection.sessions());',
            'apply_sessions_projection /* accepted replacement */ (window, projection.sessions());'
        )
        $text | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $text)

        $receipt = (& $Audit -RepositoryRoot $fixture -SourceOnly) | ConvertFrom-Json
        $receipt.sessions_projection_application_count | Should -Be 1
    }

    It "rejects untyped Sessions Next navigation" {
        $fixture = New-DesktopAuditFixture -Name "sessions-next-direction"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'DesktopSessionPageDirection::Next => reducer',
            'DesktopSessionPageDirection::Forward => reducer'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-NAVIGATION*"
    }

    It "rejects retaining a Sessions navigation queue" {
        $fixture = New-DesktopAuditFixture -Name "sessions-navigation-queue"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'pending_navigation: Option<PendingDesktopSessionPage>',
            'pending_navigation: Vec<PendingDesktopSessionPage>'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-NAVIGATION*"
    }

    It "rejects exposing a Sessions cursor through Slint" {
        $fixture = New-DesktopAuditFixture -Name "sessions-cursor-ui"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\models.slint") `
            -Value 'export struct SessionNavigationLeak { cursor: string }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-PRIVACY*"
    }

    It "rejects appending to the Sessions list model" {
        $fixture = New-DesktopAuditFixture -Name "sessions-list-append"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'window.set_session_list_rows(model(rows));',
            'rows.push(todo!()); window.set_session_list_rows(model(rows));'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-MODEL*"
    }

    It "rejects refresh that leaves a Sessions navigation active" {
        $fixture = New-DesktopAuditFixture -Name "sessions-refresh-navigation"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [regex]::Replace(
            [System.IO.File]::ReadAllText($path),
            '(?s)(work\.refresh_attempt\s*=\s*Some\(attempt\);.*?)(\s*invalidate_navigation\(&mut work\);)',
            '${1}',
            1
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-NAVIGATION*"
    }

    It "rejects a stale Sessions navigation commit" {
        $fixture = New-DesktopAuditFixture -Name "sessions-stale-navigation"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '&& reducer.snapshot().generation() == intent.product_generation()',
            '&& true'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-NAVIGATION*"
    }

    It "rejects removing the Sessions navigation epoch admission fence" {
        $fixture = New-DesktopAuditFixture -Name "sessions-navigation-epoch-fence"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'if self.snapshot_epoch() != Some(intent.snapshot_epoch()) {',
            'if false {'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-NAVIGATION*"
    }

    It "rejects removing the Sessions navigation product-generation admission fence" {
        $fixture = New-DesktopAuditFixture -Name "sessions-navigation-product-fence"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '!= Some(intent.product_generation())',
            '== Some(intent.product_generation())'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-NAVIGATION*"
    }

    It "rejects weakening the Sessions navigation monotonic admission fence" {
        $fixture = New-DesktopAuditFixture -Name "sessions-navigation-generation-fence"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'intent.navigation_generation() <= current',
            'intent.navigation_generation() < current'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-NAVIGATION*"
    }

    It "rejects changing missing Sessions continuation to a non-invalid-value failure" {
        $fixture = New-DesktopAuditFixture -Name "sessions-continuation-fail-closed"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '.ok_or(QueryErrorCode::InvalidValue)',
            '.ok_or(QueryErrorCode::Internal)'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-NAVIGATION*"
    }

    It "rejects delaying the Sessions navigation epoch stale return" {
        $fixture = New-DesktopAuditFixture -Name "sessions-navigation-epoch-return"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [regex]::Replace(
            [System.IO.File]::ReadAllText($path),
            '(if self\.snapshot_epoch\(\) != Some\(intent\.snapshot_epoch\(\)\) \{\s*)(return Err\(DesktopControllerError::new\(\s*DesktopControllerErrorCode::StaleNavigation,\s*\)\);)',
            '$1let _ = (); $2',
            1
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-NAVIGATION*"
    }

    It "rejects delaying the Sessions navigation product stale return" {
        $fixture = New-DesktopAuditFixture -Name "sessions-navigation-product-return"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [regex]::Replace(
            [System.IO.File]::ReadAllText($path),
            '(if \*lock_published_generation\(&self\.publication\.published_generation\)\?\s*!= Some\(intent\.product_generation\(\)\)\s*\{\s*)(return Err\(DesktopControllerError::new\(\s*DesktopControllerErrorCode::StaleNavigation,\s*\)\);)',
            '$1let _ = (); $2',
            1
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-NAVIGATION*"
    }

    It "rejects delaying the Sessions navigation generation stale return" {
        $fixture = New-DesktopAuditFixture -Name "sessions-navigation-generation-return"
        $path = Join-Path $fixture "crates\desktop\src\controller.rs"
        $text = [regex]::Replace(
            [System.IO.File]::ReadAllText($path),
            '(if work\s*\.navigation_high_water\s*\.is_some_and\(\|current\| intent\.navigation_generation\(\) <= current\)\s*\{\s*)(return Err\(DesktopControllerError::new\(\s*DesktopControllerErrorCode::StaleNavigation,\s*\)\);)',
            '$1let _ = (); $2',
            1
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-NAVIGATION*"
    }

    It "rejects exposing a cursor property from SessionsView" {
        $fixture = New-DesktopAuditFixture -Name "sessions-view-cursor"
        $path = Join-Path $fixture "crates\desktop\ui\views\sessions-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'export component SessionsView inherits Rectangle {',
            "export component SessionsView inherits Rectangle {`r`n    in property <string> cursor;"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-PRIVACY*"
    }

    It "rejects a public Rust Sessions cursor type" {
        $fixture = New-DesktopAuditFixture -Name "sessions-public-cursor-type"
        $path = Join-Path $fixture "crates\desktop\src\sessions.rs"
        $text = [System.IO.File]::ReadAllText($path)
        [System.IO.File]::WriteAllText($path, "$text`r`npub type DesktopSessionCursorLeak = String;")

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SESSIONS-PRIVACY*"
    }

    It "accepts a commented Sessions projection name" {
        $fixture = New-DesktopAuditFixture -Name "sessions-projection-comment"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '#[cfg(test)]',
            "// apply_sessions_projection(window, sessions);`r`n#[cfg(test)]"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Not -Throw
    }

    It "accepts an unrelated mouse cursor comment outside Sessions contracts" {
        $fixture = New-DesktopAuditFixture -Name "unrelated-mouse-cursor"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\main.slint") `
            -Value '// unrelated mouse cursor behavior'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Not -Throw
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
            -Value 'pub struct DesktopSessionLeakyDetail { key: UsageSessionKey }'

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
            'focus-on-tab-navigation: root.selection-enabled;',
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
            'apply_route_projection(window, projection);',
            "apply_route_projection(window, projection);`r`n    apply_dashboard_projection(window, projection.dashboard());"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DASHBOARD-REBUILD*"
    }

    It "rejects history model rebuilding from route selection" {
        $fixture = New-DesktopAuditFixture -Name "route-history-rebuild"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'apply_route_projection(window, projection);',
            "apply_route_projection(window, projection);`r`n    apply_history_snapshot_projection(window, projection.history());"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-HISTORY-REBUILD*"
    }

    It "rejects Models rebuilding from route selection" {
        $fixture = New-DesktopAuditFixture -Name "route-models-rebuild"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'apply_route_projection(window, projection);',
            "apply_route_projection(window, projection);`r`n    apply_models_projection(window, projection.models());"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-MODELS-REBUILD*"
    }

    It "rejects Projects rebuilding from route selection" {
        $fixture = New-DesktopAuditFixture -Name "route-projects-rebuild"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'apply_route_projection(window, projection);',
            "apply_route_projection(window, projection);`r`n    apply_projects_projection(window, projection.projects());"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PROJECTS-REBUILD*"
    }

    It "rejects sessions model rebuilding from route selection" {
        $fixture = New-DesktopAuditFixture -Name "route-sessions-rebuild"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'apply_route_projection(window, projection);',
            "apply_route_projection(window, projection);`r`n    apply_sessions_projection(window, projection.sessions());"
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

    It "rejects a second command palette route model" {
        $fixture = New-DesktopAuditFixture -Name "command-palette-second-model"
        $path = Join-Path $fixture "crates\desktop\ui\main.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'in property <[RouteRow]> command-palette-rows;',
            "in property <[RouteRow]> command-palette-rows;`n    in property <[RouteRow]> command-palette-rows;"
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

    It "rejects rewiring command palette open callbacks" {
        $fixture = New-DesktopAuditFixture -Name "command-palette-open-action"
        $path = Join-Path $fixture "crates\desktop\ui\main.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'root.open-command-palette();',
            'root.dismiss-command-palette();'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-COMMAND-PALETTE-SHORTCUT*"
    }

    It "rejects rewiring command palette Escape" {
        $fixture = New-DesktopAuditFixture -Name "command-palette-escape-action"
        $path = Join-Path $fixture "crates\desktop\ui\components\command-palette.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'root.dismiss();',
            'root.move-selection(1);'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-COMMAND-PALETTE-SHORTCUT*"
    }

    It "rejects rewiring command palette Up" {
        $fixture = New-DesktopAuditFixture -Name "command-palette-up-action"
        $path = Join-Path $fixture "crates\desktop\ui\components\command-palette.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'root.move-selection(-1);',
            'root.move-selection(1);'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-COMMAND-PALETTE-SHORTCUT*"
    }

    It "rejects rewiring command palette Down" {
        $fixture = New-DesktopAuditFixture -Name "command-palette-down-action"
        $path = Join-Path $fixture "crates\desktop\ui\components\command-palette.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'root.move-selection(1);',
            'root.move-selection(-1);'
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

    It "rejects rewiring the command palette accessible default action" {
        $fixture = New-DesktopAuditFixture -Name "command-palette-accessible-action"
        $path = Join-Path $fixture "crates\desktop\ui\components\command-palette.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'accessible-action-default => { root.activate-route(route.key); }',
            'accessible-action-default => { root.dismiss(); }'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-COMMAND-PALETTE-ROUTE-ONLY*"
    }

    It "rejects rewiring command palette pointer activation" {
        $fixture = New-DesktopAuditFixture -Name "command-palette-pointer-action"
        $path = Join-Path $fixture "crates\desktop\ui\components\command-palette.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'TouchArea { clicked => { root.activate-route(route.key); } }',
            'TouchArea { clicked => { root.dismiss(); } }'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-COMMAND-PALETTE-ROUTE-ONLY*"
    }

    It "rejects rewiring command palette Enter activation" {
        $fixture = New-DesktopAuditFixture -Name "command-palette-enter-action"
        $path = Join-Path $fixture "crates\desktop\ui\components\command-palette.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'root.activate-selection();',
            'root.dismiss();'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-COMMAND-PALETTE-SHORTCUT*"
    }

    It "rejects removing the command palette ancestor focus scope" {
        $fixture = New-DesktopAuditFixture -Name "command-palette-overlay-ancestor"
        $path = Join-Path $fixture "crates\desktop\ui\main.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'shell-focus := FocusScope {',
            'shell-focus := Rectangle {'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-COMMAND-PALETTE-OVERLAY*"
    }

    It "rejects moving the command palette before the notification layer" {
        $fixture = New-DesktopAuditFixture -Name "command-palette-overlay-order"
        $path = Join-Path $fixture "crates\desktop\ui\main.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '    RoutePalette {',
            '    MovedPalette {'
        ).Replace(
            '    if root.in-app-notification-visible: InAppNotificationPanel {',
            "    RoutePalette { }`n    if root.in-app-notification-visible: InAppNotificationPanel {"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-COMMAND-PALETTE-OVERLAY*"
    }

    It "rejects a second compact quota model" {
        $fixture = New-DesktopAuditFixture -Name "compact-second-quota-model"
        $path = Join-Path $fixture "crates\desktop\ui\main.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'in property <[DashboardQuotaRow]> dashboard-quota-rows;',
            "in property <[DashboardQuotaRow]> dashboard-quota-rows;`n    in property <[DashboardQuotaRow]> compact-quota-rows;"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-COMPACT-QUOTA*"
    }

    It "rejects fixed weekly compact quota assumptions" {
        $fixture = New-DesktopAuditFixture -Name "compact-fixed-weekly"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\views\compact-widget-view.slint") `
            -Value 'Text { text: "Weekly quota"; }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*QUOTA*"
    }

    It "rejects hiding an unknown compact quota ratio" {
        $fixture = New-DesktopAuditFixture -Name "compact-unknown-ratio"
        $path = Join-Path $fixture "crates\desktop\ui\views\compact-widget-view.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'Usage ratio unavailable',
            '0% used'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-COMPACT-QUOTA*"
    }

    It "rejects rewiring compact return away from Dashboard" {
        $fixture = New-DesktopAuditFixture -Name "compact-return-route"
        $path = Join-Path $fixture "crates\desktop\ui\main.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'return-dashboard => { root.select-route("dashboard"); }',
            'return-dashboard => { root.select-route("settings"); }'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-COMPACT-ROUTE*"
    }

    It "rejects conditionally reconstructing the compact view" {
        $fixture = New-DesktopAuditFixture -Name "compact-conditional-view"
        $path = Join-Path $fixture "crates\desktop\ui\main.slint"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'compact-view := CompactWidgetView {',
            'if root.compact-widget-visible: CompactWidgetView {'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-COMPACT-ROUTE*"
    }

    It "rejects removing the compact restore-size slot" {
        $fixture = New-DesktopAuditFixture -Name "compact-geometry-slot"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'normal_size: Option<slint::PhysicalSize>',
            'discarded_size: Option<slint::PhysicalSize>'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-COMPACT-GEOMETRY*"
    }

    It "rejects adding a compact runtime owner" {
        $fixture = New-DesktopAuditFixture -Name "compact-runtime-owner"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\ui.rs") `
            -Value 'struct CompactWidgetWorker;'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-COMPACT-NO-OWNER*"
    }

    It "rejects a second production tray owner" {
        $fixture = New-DesktopAuditFixture -Name "tray-second-component"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\native_tray.rs") `
            -Value 'fn duplicate_owner() { let _ = CreateWindowExW(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-TRAY-SURFACE*"
    }

    It "rejects removing a typed tray action" {
        $fixture = New-DesktopAuditFixture -Name "tray-missing-action"
        $path = Join-Path $fixture "crates\desktop\src\native_tray.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'COMMAND_COMPACT => Some(DesktopLifecycleIntent::OpenCompact),',
            'COMMAND_COMPACT => None,'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-TRAY-INTENT*"
    }

    It "rejects rewiring a tray click away from Show" {
        $fixture = New-DesktopAuditFixture -Name "tray-click-drift"
        $path = Join-Path $fixture "crates\desktop\src\native_tray.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'inner.submit(DesktopLifecycleIntent::Show);',
            'inner.submit(DesktopLifecycleIntent::Hide);'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-TRAY-SURFACE*"
    }

    It "rejects replacing the queue-free lifecycle router slot" {
        $fixture = New-DesktopAuditFixture -Name "tray-router-queue"
        $path = Join-Path $fixture "crates\desktop\src\shell.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'sink: RefCell<Option<Rc<dyn DesktopLifecycleIntentSink>>>',
            'sink: RefCell<Vec<Rc<dyn DesktopLifecycleIntentSink>>>'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-TRAY-INTENT*"
    }

    It "rejects adding a tray runtime owner" {
        $fixture = New-DesktopAuditFixture -Name "tray-runtime-owner"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\native_tray.rs") `
            -Value 'fn tray_poll() { Timer::default(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-TRAY-LIFECYCLE*"
    }

    It "rejects an unverified native tray callback binding" {
        $fixture = New-DesktopAuditFixture -Name "tray-callback-binding-drift"
        $path = Join-Path $fixture "crates\desktop\src\native_tray.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'let installed = unsafe { GetWindowLongPtrW(inner.hwnd, GWLP_USERDATA) };',
            'let installed = callback_state as isize;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-TRAY-RECOVERY*"
    }

    It "rejects removing close-to-tray interception" {
        $fixture = New-DesktopAuditFixture -Name "tray-close-drift"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'DesktopCloseEffect::Quit => {',
            'DesktopCloseEffect::HideToTray => {'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-TRAY-LIFECYCLE*"
    }

    It "rejects production tray icon drift" {
        $fixture = New-DesktopAuditFixture -Name "tray-icon-drift"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\assets\tokenmaster-tray-color-32.svg") `
            -Value '<!-- drift -->'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-TRAY-ASSET*"
    }

    It "accepts the bounded dashboard History Sessions Models Projects Activity Notifications and Help About desktop boundary" {
        $fixture = New-DesktopAuditFixture -Name "library-boundary"

        $receipt = & $Audit -RepositoryRoot $fixture -SourceOnly | ConvertFrom-Json
        $receipt.rust_source_file_count | Should -Be 18
        $receipt.slint_source_file_count | Should -Be 24
        $receipt.density_variant_count | Should -Be 3
        $receipt.density_stable_key_arm_count | Should -Be 3
        $receipt.density_slint_index_arm_count | Should -Be 3
        $receipt.density_from_slint_index_arm_count | Should -Be 3
        $receipt.density_token_table_count | Should -Be 7
        $receipt.density_owner_count | Should -Be 1
        $receipt.density_owner_slot_count | Should -Be 1
        $receipt.density_root_binding_count | Should -Be 1
        $receipt.density_root_callback_count | Should -Be 1
        $receipt.density_wiring_callback_count | Should -Be 1
        $receipt.density_revision_type_count | Should -Be 1
        $receipt.density_checked_successor_count | Should -Be 1
        $receipt.density_successor_call_count | Should -Be 1
        $receipt.density_write_count | Should -Be 1
        $receipt.density_revision_write_count | Should -Be 1
        $receipt.density_switch_loop_count | Should -Be 1
        $receipt.presentation_operation_switch_loop_count | Should -Be 1
        $receipt.presentation_ui_switch_structure_sha256 | Should -Be '0f8e1e7cc0bc9ed225d7dcbc338e2e464689c84812d75a5f4ae479463e20d429'
        $receipt.density_applied_assertion_count | Should -Be 1
        $receipt.density_final_postcondition_count | Should -Be 1
        $receipt.density_authority_count | Should -Be 0
        $receipt.density_allowed_owner_occurrence_count | Should -Be 1
        $receipt.density_allowed_owner_wire_signature_count | Should -Be 1
        $receipt.density_authority_timer_delay_interval_sleep_count | Should -Be 0
        $receipt.density_authority_worker_thread_spawn_task_count | Should -Be 0
        $receipt.density_authority_query_count | Should -Be 0
        $receipt.density_authority_window_create_count | Should -Be 0
        $receipt.density_authority_queue_deque_count | Should -Be 0
        $receipt.density_authority_cache_count | Should -Be 0
        $receipt.density_authority_channel_count | Should -Be 0
        $receipt.density_authority_unsafe_count | Should -Be 0
        $receipt.density_authority_retained_count | Should -Be 0
        $receipt.skin_variant_count | Should -Be 3
        $receipt.skin_key_mapping_count | Should -Be 3
        $receipt.skin_index_mapping_count | Should -Be 3
        $receipt.skin_reverse_index_mapping_count | Should -Be 3
        $receipt.palette_role_count | Should -Be 15
        $receipt.palette_exact_rgb_value_count | Should -Be 45
        $receipt.palette_slot_count | Should -Be 1
        $receipt.palette_property_count | Should -Be 2
        $receipt.palette_struct_count | Should -Be 1
        $receipt.skin_family_callback_count | Should -Be 2
        $receipt.presentation_callback_count | Should -Be 4
        $receipt.skin_root_callback_count | Should -Be 1
        $receipt.skin_settings_callback_count | Should -Be 1
        $receipt.skin_forward_binding_count | Should -Be 1
        $receipt.skin_wiring_callback_count | Should -Be 1
        $receipt.palette_order_count | Should -Be 1
        $receipt.command_palette_query_scalar_maximum | Should -Be 64
        $receipt.command_palette_model_count | Should -Be 1
        $receipt.command_palette_shortcut_count | Should -Be 1
        $receipt.command_palette_accessible_default_action_count | Should -Be 1
        $receipt.command_palette_route_only | Should -BeTrue
        $receipt.command_palette_owner_count | Should -Be 0
        $receipt.compact_widget_quota_row_maximum | Should -Be 32
        $receipt.compact_widget_quota_model_count | Should -Be 1
        $receipt.compact_widget_geometry_slot_count | Should -Be 1
        $receipt.compact_widget_owner_count | Should -Be 0
        $receipt.tray_component_count | Should -Be 1
        $receipt.tray_intent_count | Should -Be 5
        $receipt.tray_router_slot_count | Should -Be 1
        $receipt.tray_close_handler_count | Should -Be 1
        $receipt.tray_owner_count | Should -Be 1
        $receipt.tray_explorer_recovery_count | Should -Be 1
        $receipt.tray_readd_check_count | Should -Be 1
        $receipt.tray_callback_binding_count | Should -Be 1
        $receipt.tray_focus_count | Should -Be 1
        $receipt.tray_polling_surface_count | Should -Be 0
        $receipt.tray_icon_sha256 | Should -Be '1782E746EFBB423DF3252FD76B9E9E7135416DA966DF0C5652588AC29C0A6246'
        $receipt.dashboard_section_count | Should -Be 6
        $receipt.dashboard_model_replacement_count | Should -Be 7
        $receipt.history_day_maximum | Should -Be 30
        $receipt.history_model_replacement_count | Should -Be 1
        $receipt.history_projection_application_count | Should -Be 1
        $receipt.model_row_maximum | Should -Be 64
        $receipt.models_model_replacement_count | Should -Be 1
        $receipt.models_projection_application_count | Should -Be 1
        $receipt.analytics_query_call_count | Should -Be 3
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

    It "rejects removing a fixed density key" {
        $fixture = New-DesktopAuditFixture -Name "density-key-drift"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'Self::UltraCompact => "ultra_compact",',
            'Self::UltraCompact => "ultra",'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-CONTRACT*"
    }

    It "rejects widening a fixed density index" {
        $fixture = New-DesktopAuditFixture -Name "density-index-drift"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '2 => Some(Self::UltraCompact),',
            '3 => Some(Self::UltraCompact),'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-CONTRACT*"
    }

    It "rejects a fourth density mapping arm" {
        $fixture = New-DesktopAuditFixture -Name "density-fourth-mapping"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '    UltraCompact,',
            "    UltraCompact,`r`n    ExtraCompact,"
        ).Replace(
            '            Self::UltraCompact => "ultra_compact",',
            "            Self::UltraCompact => `"ultra_compact`",`r`n            Self::ExtraCompact => `"extra_compact`","
        ).Replace(
            '            Self::UltraCompact => 2,',
            "            Self::UltraCompact => 2,`r`n            Self::ExtraCompact => 3,"
        ).Replace(
            '            2 => Some(Self::UltraCompact),',
            "            2 => Some(Self::UltraCompact),`r`n            3 => Some(Self::ExtraCompact),"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-CONTRACT*"
    }

    It "rejects an eighth density token table" {
        $fixture = New-DesktopAuditFixture -Name "density-eighth-token"
        $path = Join-Path $fixture "crates\desktop\ui\tokens.slint"
        $text = [System.IO.File]::ReadAllText($path) -replace '(\r?\n\})\s*$', "`r`n    out property <length> density-extra: density-id == 2 ? 1px : (density-id == 1 ? 2px : 3px);`$1"
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-TOKENS*"
    }

    It "rejects unchecked presentation revision updates" {
        $fixture = New-DesktopAuditFixture -Name "density-revision-drift"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'self.0.checked_add(1)',
            'self.0.saturating_add(1)'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-REVISION*"
    }

    It "rejects a dead checked_add marker with an unchecked revision path" {
        $fixture = New-DesktopAuditFixture -Name "density-dead-checked-add"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'match self.0.checked_add(1) {',
            'match Some(self.0.wrapping_add(1)) {'
        )
        $text += "`r`nfn dead_checked_add_marker() { let _ = 0_u64.checked_add(1); }"
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-REVISION*"
    }

    It "rejects a computed successor without revision assignment" {
        $fixture = New-DesktopAuditFixture -Name "density-missing-revision-assignment"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '        self.revision = revision;',
            '        let _ = revision;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-REVISION*"
    }

    It "rejects removing the semantic 10,000-switch outcome assertion" {
        $fixture = New-DesktopAuditFixture -Name "density-stress-outcome"
        $path = Join-Path $fixture "crates\desktop\tests\presentation_style_contract.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'DesktopPresentationApplyOutcome::Applied',
            'DesktopPresentationApplyOutcome::Unchanged'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-STRESS*"
    }

    It "rejects density timer authority" {
        $fixture = New-DesktopAuditFixture -Name "density-authority-timer"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        Add-Content -LiteralPath $path -Value 'fn density_timer() { slint::Timer::default(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-NO-AUTHORITY*"
    }

    It "rejects density thread spawn authority" {
        $fixture = New-DesktopAuditFixture -Name "density-authority-thread"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        Add-Content -LiteralPath $path -Value 'fn density_worker() { std::thread::spawn(|| {}); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-NO-AUTHORITY*"
    }

    It "rejects density query authority" {
        $fixture = New-DesktopAuditFixture -Name "density-authority-query"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        Add-Content -LiteralPath $path -Value 'fn density_query() { QueryService::new(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-NO-AUTHORITY*"
    }

    It "rejects density native window creation" {
        $fixture = New-DesktopAuditFixture -Name "density-authority-window"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        Add-Content -LiteralPath $path -Value 'fn density_window() { CreateWindowExW(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-NO-AUTHORITY*"
    }

    It "rejects density queue authority" {
        $fixture = New-DesktopAuditFixture -Name "density-authority-queue"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        Add-Content -LiteralPath $path -Value 'fn density_queue() { VecDeque::<u8>::new(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-NO-AUTHORITY*"
    }

    It "rejects density cache authority" {
        $fixture = New-DesktopAuditFixture -Name "density-authority-cache"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        Add-Content -LiteralPath $path -Value 'fn density_cache() { DensityCache::new(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-NO-AUTHORITY*"
    }

    It "rejects density channel authority" {
        $fixture = New-DesktopAuditFixture -Name "density-authority-channel"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        Add-Content -LiteralPath $path -Value 'fn density_channel() { std::sync::mpsc::sync_channel::<u8>(1); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-NO-AUTHORITY*"
    }

    It "rejects density unsafe authority" {
        $fixture = New-DesktopAuditFixture -Name "density-authority-unsafe"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        Add-Content -LiteralPath $path -Value 'fn density_unsafe() { unsafe {} }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-NO-AUTHORITY*"
    }

    It "rejects density retained synchronization authority" {
        $fixture = New-DesktopAuditFixture -Name "density-authority-sync"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        Add-Content -LiteralPath $path -Value 'fn density_sync() { Mutex::<u8>::new(0); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-NO-AUTHORITY*"
    }

    It "rejects a fourth mapping arm hidden after lexical brace decoys" {
        $fixture = New-DesktopAuditFixture -Name "density-lexical-fourth-mapping"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '    UltraCompact,',
            "    UltraCompact,`r`n    // }`r`n    /* { /* } */ */`r`n    ExtraCompact,"
        ).Replace(
            '            Self::UltraCompact => "ultra_compact",',
            "            Self::UltraCompact => `"ultra_compact`",`r`n            let _ = `"}`";`r`n            Self::ExtraCompact => `"extra_compact`","
        ).Replace(
            '            Self::UltraCompact => 2,',
            "            Self::UltraCompact => 2,`r`n            let _ = r###`"}`"###;`r`n            Self::ExtraCompact => 3,"
        ).Replace(
            '            2 => Some(Self::UltraCompact),',
            "            2 => Some(Self::UltraCompact),`r`n            let _ = '}';`r`n            3 => Some(Self::ExtraCompact),"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-CONTRACT*"
    }

    It "rejects a dead in-function revision structure despite loose markers" {
        $fixture = New-DesktopAuditFixture -Name "density-lexical-revision-structure"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'match self.0.checked_add(1) {',
            'match Some(self.0.wrapping_add(1)) {'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-REVISION*"
    }

    It "rejects a dead stress marker when the 10,000 loop body no longer asserts Applied" {
        $fixture = New-DesktopAuditFixture -Name "density-lexical-stress-structure"
        $path = Join-Path $fixture "crates\desktop\tests\presentation_style_contract.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'DesktopPresentationApplyOutcome::Applied',
            'DesktopPresentationApplyOutcome::Unchanged'
        ).Replace(
            '    assert_eq!(style.density(), DesktopDensity::Comfortable);',
            "    if false { assert_eq!(style.select_density_index(0), DesktopPresentationApplyOutcome::Applied); }`r`n    // for index in 0..10_000 { }`r`n    assert_eq!(style.density(), DesktopDensity::Comfortable);"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-STRESS*"
    }

    It "rejects an eighth multiline UiTokens density property" {
        $fixture = New-DesktopAuditFixture -Name "density-lexical-multiline-token"
        $path = Join-Path $fixture "crates\desktop\ui\tokens.slint"
        $text = [System.IO.File]::ReadAllText($path) -replace '(\r?\n\})\s*$', "`r`n    out property <length> density-extra:`r`n        density-id == 2 ? 1px : (density-id == 1 ? 2px : 3px);`$1"
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-TOKENS*"
    }

    It "rejects scoped thread spawn authority in the density presentation body" {
        $fixture = New-DesktopAuditFixture -Name "density-lexical-scoped-thread"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        Add-Content -LiteralPath $path -Value 'fn density_scope() { std::thread::scope(|s| s.spawn(|| {})); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-NO-AUTHORITY*"
    }

    It "rejects authority after comment string raw-string and character brace decoys in apply" {
        $fixture = New-DesktopAuditFixture -Name "density-lexical-apply-authority"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '    window.set_presentation_revision(style.revision().get().to_string().into());',
            "    window.set_presentation_revision(style.revision().get().to_string().into());`r`n    // }`r`n    let _ = `"}`";`r`n    let _ = r###`"}`"###;`r`n    let _ = '}';`r`n    worker::new();"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-NO-AUTHORITY*"
    }

    It "ignores authority markers in density comments and literals" {
        $fixture = New-DesktopAuditFixture -Name "density-lexical-non-executable-markers"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\presentation_style.rs") `
            -Value '// std::thread::scope(|s| s.spawn(|| {})); Timer::default(); }'
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\presentation_style.rs") `
            -Value 'const DENSITY_MARKER: &str = "unsafe VecDeque QueryService }";'
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\ui\tokens.slint") `
            -Value '// out property <length> density-fake: density-id == 2 ? 1px : 2px;'

        $receipt = & $Audit -RepositoryRoot $fixture -SourceOnly | ConvertFrom-Json
        $receipt.density_authority_count | Should -Be 0
        $receipt.density_token_table_count | Should -Be 7
    }

    It "rejects lowercase worker and retained collection constructors" {
        $fixture = New-DesktopAuditFixture -Name "density-lexical-retained-authority"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        Add-Content -LiteralPath $path -Value 'fn density_authority() { let _ = worker::new(); let _ = Box::new(1); let _ = Vec::<u8>::new(); let _ = HashMap::<u8, u8>::new(); let _ = OnceLock::<u8>::new(); let _ = Cell::new(0); let _ = Rc::new(0); let _ = RefCell::new(0); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-NO-AUTHORITY*"
    }

    It "rejects multiline channel queue cache and query authority" {
        $fixture = New-DesktopAuditFixture -Name "density-lexical-multiline-authority"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        Add-Content -LiteralPath $path -Value @'
fn density_authority() {
    let _ = std::sync::mpsc::
        channel::<u8>();
    let _ = VecDeque::<u8>::new();
    let _ = DensityCache::new();
    let _ = QueryService::new();
}
'@

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-NO-AUTHORITY*"
    }

    It "rejects authority after a zero-hash raw string brace in apply" {
        $fixture = New-DesktopAuditFixture -Name "density-zero-hash-raw-apply"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '    window.set_presentation_revision(style.revision().get().to_string().into());',
            "    window.set_presentation_revision(style.revision().get().to_string().into());`r`n    let _ = r`"}`";`r`n    worker::new();"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-NO-AUTHORITY*"
    }

    It "rejects authority after a zero-hash byte-raw string brace in wire" {
        $fixture = New-DesktopAuditFixture -Name "density-zero-hash-byte-raw-wire"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '    let weak_window = window.as_weak();',
            "    let weak_window = window.as_weak();`r`n    let _ = br`"}`";`r`n    worker::new();"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-NO-AUTHORITY*"
    }

    It "rejects authority after a byte string brace in apply" {
        $fixture = New-DesktopAuditFixture -Name "density-byte-string-apply"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '    window.set_presentation_revision(style.revision().get().to_string().into());',
            "    window.set_presentation_revision(style.revision().get().to_string().into());`r`n    let _ = b`"}`";`r`n    worker::new();"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-NO-AUTHORITY*"
    }

    It "rejects authority after a byte character brace in wire" {
        $fixture = New-DesktopAuditFixture -Name "density-byte-character-wire"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '    let weak_window = window.as_weak();',
            "    let weak_window = window.as_weak();`r`n    let _ = b'}';`r`n    worker::new();"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-NO-AUTHORITY*"
    }

    It "rejects a second identical density style owner signature" {
        $fixture = New-DesktopAuditFixture -Name "density-second-identical-owner"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        Add-Content -LiteralPath $path -Value 'fn duplicate_owner(_: Rc<RefCell<DesktopPresentationStyle>>) {}'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-NO-AUTHORITY*"
    }

    It "rejects Once retained authority" {
        $fixture = New-DesktopAuditFixture -Name "density-once-authority"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        Add-Content -LiteralPath $path -Value 'fn density_once() { Once::new(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-NO-AUTHORITY*"
    }

    It "rejects a non-u64 revision wrapper hidden by comment markers" {
        $fixture = New-DesktopAuditFixture -Name "density-commented-revision-wrapper"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'pub struct DesktopPresentationRevision(u64);',
            "// pub struct DesktopPresentationRevision(u64);`r`npub struct DesktopPresentationRevision(u32);"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-REVISION*"
    }

    It "rejects applying presentation density before intent admission" {
        $fixture = New-DesktopAuditFixture -Name "density-before-admission"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '    if selected.select_density_index_if_admitted(index, |selection| {',
            '    let _ = selected.select_density_index(index);' + "`r`n" +
            '    if selected.select_density_index_if_admitted(index, |selection| {'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-ADMISSION*"
    }

    It "rejects a second density worker owner" {
        $fixture = New-DesktopAuditFixture -Name "density-second-worker"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\presentation_style.rs") `
            -Value 'fn second_density_worker() { std::thread::spawn(|| {}); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-NO-AUTHORITY*"
    }

    It "rejects removing the latest-payload coalescing proof" {
        $fixture = New-DesktopAuditFixture -Name "density-missing-latest-payload-proof"
        $path = Join-Path $fixture "crates\app\src\operation_tests.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'ten_thousand_presentation_updates_keep_one_latest_payload',
            'coverage_removed'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-STRESS*"
    }

    It "rejects reminder and backup updates that discard presentation" {
        $fixture = New-DesktopAuditFixture -Name "presentation-discarded-by-settings-update"
        $path = Join-Path $fixture "crates\app\src\state.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '*current.value().portable().presentation(),',
            'PresentationSettings::refined(),'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PRESENTATION-PRESERVATION*"
    }

    It "rejects density assignment inside the admitted callee before admission" {
        $fixture = New-DesktopAuditFixture -Name "density-callee-before-admission"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        $text = [System.IO.File]::ReadAllText($path)
        $newline = if ($text.Contains("`r`n")) { "`r`n" } else { "`n" }
        $text = $text.Replace(
            '        if !admit(selection) {',
            '        self.selection = selection;' + $newline + '        if !admit(selection) {'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-ADMISSION*"
    }

    It "rejects empty active and pending coalescing assertions while test names remain" {
        $fixture = New-DesktopAuditFixture -Name "density-empty-coalescing-assertions"
        $path = Join-Path $fixture "crates\app\src\operation_tests.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '        assert_eq!(snapshot.active_count(), 1);',
            '        let _ = snapshot.active_count();'
        ).Replace(
            '        assert_eq!(snapshot.pending_count(), 1);',
            '        let _ = snapshot.pending_count();'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-STRESS*"
    }

    It "rejects loss of the final coalesced payload assertion while the test name remains" {
        $fixture = New-DesktopAuditFixture -Name "density-missing-final-payload-assertion"
        $path = Join-Path $fixture "crates\app\src\operation_tests.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '    assert_eq!(receive(&started_rx), final_selection);',
            '    let _ = receive(&started_rx);'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-STRESS*"
    }

    It "rejects a fourth skin variant" {
        $fixture = New-DesktopAuditFixture -Name "skin-fourth"
        $path = Join-Path $fixture "crates\desktop\src\skin.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '    Ember,',
            '    Ember,`r`n    Aurora,'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SKIN-CONTRACT*"
    }

    It "rejects a changed executable skin key despite a comment decoy" {
        $fixture = New-DesktopAuditFixture -Name "skin-key-comment-decoy"
        $path = Join-Path $fixture "crates\\desktop\\src\\skin.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'Self::Refined => "refined",',
            "// Self::Refined => `"refined`",`r`n            Self::Refined => `"polished`","
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SKIN-CONTRACT*"
    }

    It "rejects a missing palette role" {
        $fixture = New-DesktopAuditFixture -Name "skin-missing-palette-role"
        $skinPath = Join-Path $fixture "crates\desktop\src\skin.rs"
        $text = [System.IO.File]::ReadAllText($skinPath).Replace('    unavailable: DesktopRgb,', '')
        [System.IO.File]::WriteAllText($skinPath, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SKIN-PALETTE*"
    }

    It "rejects a Slint skin family table" {
        $fixture = New-DesktopAuditFixture -Name "skin-family-table"
        $tokensPath = Join-Path $fixture "crates\desktop\ui\tokens.slint"
        Add-Content -LiteralPath $tokensPath -Value 'property <color> graphite-family: #000000;'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SKIN-PALETTE*"
    }

    It "rejects a stable skin mapping drift" {
        $fixture = New-DesktopAuditFixture -Name "skin-index-drift"
        $path = Join-Path $fixture "crates\desktop\src\skin.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace('Self::Graphite => 1,', 'Self::Graphite => 9,')
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SKIN-CONTRACT*"
    }

    It "rejects a second palette owner" {
        $fixture = New-DesktopAuditFixture -Name "skin-second-owner"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'fn ui_palette(skin: crate::DesktopSkin) -> UiPalette {',
            "fn duplicate_skin_owner(_: Arc<Mutex<DesktopPresentationStyle>>) {}`r`n`r`nfn ui_palette(skin: crate::DesktopSkin) -> UiPalette {"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PRESENTATION-OWNER*"
    }

    It "rejects yielding between the Rust palette and presentation metadata" {
        $fixture = New-DesktopAuditFixture -Name "skin-yield"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path)
        $newline = if ($text.Contains("`r`n")) { "`r`n" } else { "`n" }
        $text = $text.Replace(
            '    window.set_presentation_skin_id(style.skin().slint_index());',
            '    window.set_presentation_skin_id(style.skin().slint_index());' + $newline +
            '    slint::invoke_from_event_loop(|| {}).unwrap();'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PRESENTATION-ORDER*"
    }

    It "rejects one RGB component drift and a comment palette decoy" {
        $fixture = New-DesktopAuditFixture -Name "skin-rgb-comment-decoy"
        $path = Join-Path $fixture "crates\\desktop\\src\\skin.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'background: rgb(11, 15, 23),',
            "// background: rgb(11, 15, 23),`r`n        background: rgb(12, 15, 23),"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SKIN-PALETTE*"
    }

    It "rejects skin application before complete-pair admission" {
        $fixture = New-DesktopAuditFixture -Name "skin-before-admission"
        $path = Join-Path $fixture "crates\\desktop\\src\\ui.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '    if selected.select_skin_index_if_admitted(index, |selection| {',
            '    let _ = selected.select_skin_index(index);' + "`r`n" +
            '    if selected.select_skin_index_if_admitted(index, |selection| {'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-ADMISSION*"
    }

    It "rejects a second differently named skin callback" {
        $fixture = New-DesktopAuditFixture -Name "skin-extra-callback"
        $path = Join-Path $fixture "crates\desktop\ui\views\settings-view.slint"
        $text = [System.IO.File]::ReadAllText($path)
        $original = 'export component SettingsView inherits Rectangle {'
        $replacement = $original + [Environment]::NewLine +
            '    callback select-presentation-theme(int);'
        ([regex]::Matches($text, [regex]::Escape($original))).Count | Should -Be 1
        [System.IO.File]::WriteAllText($path, $text.Replace($original, $replacement))
        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PRESENTATION-OWNER*"
    }

    It "rejects a second differently named UiPalette slot" {
        $fixture = New-DesktopAuditFixture -Name "skin-extra-slot"
        $path = Join-Path $fixture "crates\desktop\ui\main.slint"
        $text = [System.IO.File]::ReadAllText($path)
        $newline = if ($text.Contains("`r`n")) { "`r`n" } else { "`n" }
        $text = $text.Replace(
            'in-out property <UiPalette> presentation-palette <=> UiTokens.palette;',
            'in-out property <UiPalette> presentation-palette <=> UiTokens.palette;' + $newline +
            '    in-out property <UiPalette> family-palette: UiTokens.palette;'
        )
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PRESENTATION-OWNER*"
    }

    It "rejects a sixteenth palette alias" {
        $fixture = New-DesktopAuditFixture -Name "skin-extra-alias"
        $path = Join-Path $fixture "crates\desktop\ui\tokens.slint"
        $text = [System.IO.File]::ReadAllText($path)
        $newline = if ($text.Contains("`r`n")) { "`r`n" } else { "`n" }
        $text = $text.Replace(
            '    out property <length> space-xs:',
            '    out property <color> alternate: palette.accent;' + $newline +
            '    out property <length> space-xs:'
        )
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PRESENTATION-OWNER*"
    }

    It "rejects showing before the initial palette application" {
        $fixture = New-DesktopAuditFixture -Name "skin-show-before-apply"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path)
        $newline = if ($text.Contains("`r`n")) { "`r`n" } else { "`n" }
        $text = $text.Replace(
            'let window = MainWindow::new()?;',
            'let window = MainWindow::new()?;' + $newline + '        window.show()?;'
        )
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PRESENTATION-ORDER*"
    }

    It "rejects stable-key drift despite a valid literal decoy" {
        $fixture = New-DesktopAuditFixture -Name "skin-literal-key-drift"
        $path = Join-Path $fixture "crates\desktop\src\skin.rs"
        $text = [System.IO.File]::ReadAllText($path)
        $newline = if ($text.Contains("`r`n")) { "`r`n" } else { "`n" }
        $text = $text.Replace(
            'impl DesktopSkin {',
            'const SKIN_KEY_DECOY: &str = "Self::Refined => \"refined\",";' +
            $newline + $newline + 'impl DesktopSkin {'
        ).Replace('Self::Refined => "refined",', 'Self::Refined => "polished",')
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SKIN-CONTRACT*"
    }

    It "rejects reverse-index mapping drift in otherwise valid Rust" {
        $fixture = New-DesktopAuditFixture -Name "skin-reverse-index-drift"
        $path = Join-Path $fixture "crates\desktop\src\skin.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '2 => Some(Self::Ember),',
            '2 => Some(Self::Graphite),'
        )
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-SKIN-CONTRACT*"
    }

    It "rejects a second UiPalette slot owned by UiTokens" {
        $fixture = New-DesktopAuditFixture -Name "skin-uitokens-extra-slot"
        $path = Join-Path $fixture "crates\desktop\ui\tokens.slint"
        $text = [System.IO.File]::ReadAllText($path)
        $original = '    in-out property <int> density-id: 0;'
        $replacement = '    in-out property <UiPalette> family-palette: palette;' +
            [Environment]::NewLine + $original
        ([regex]::Matches($text, [regex]::Escape($original))).Count | Should -Be 1
        [System.IO.File]::WriteAllText($path, $text.Replace($original, $replacement))

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PRESENTATION-OWNER*"
    }

    It "rejects a skin-family callback owned by another component" {
        $fixture = New-DesktopAuditFixture -Name "skin-other-component-callback"
        $path = Join-Path $fixture "crates\desktop\ui\views\activity-view.slint"
        $text = [System.IO.File]::ReadAllText($path)
        $original = 'export component ActivityView inherits Rectangle {'
        $replacement = $original + [Environment]::NewLine +
            '    callback select-presentation-theme(int);'
        ([regex]::Matches($text, [regex]::Escape($original))).Count | Should -Be 1
        [System.IO.File]::WriteAllText($path, $text.Replace($original, $replacement))

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PRESENTATION-OWNER*"
    }

    It "rejects another style mutator before complete-pair admission" {
        $fixture = New-DesktopAuditFixture -Name "skin-other-mutator-before-admission"
        $path = Join-Path $fixture "crates\desktop\src\ui.rs"
        $text = [System.IO.File]::ReadAllText($path)
        $original = '    if selected.select_skin_index_if_admitted(index, |selection| {'
        $replacement = '    let _ = selected.apply_persisted_override(captured.selection());' +
            [Environment]::NewLine + $original
        ([regex]::Matches($text, [regex]::Escape($original))).Count | Should -Be 1
        [System.IO.File]::WriteAllText($path, $text.Replace($original, $replacement))

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-ADMISSION*"
    }

    It "rejects weakening the compiled mixed-axis ten-thousand switch proof" {
        $fixture = New-DesktopAuditFixture -Name "skin-ui-switch-loop-weakened"
        $path = Join-Path $fixture "crates\desktop\tests\presentation_skin_ui_contract.rs"
        $text = [System.IO.File]::ReadAllText($path)
        $original = '    for index in 0..10_000 {'
        ([regex]::Matches($text, [regex]::Escape($original))).Count | Should -Be 1
        [System.IO.File]::WriteAllText($path, $text.Replace($original, '    for index in 0..1_000 {'))

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-STRESS*"
    }

    It "rejects removing the select transition revision write" {
        $fixture = New-DesktopAuditFixture -Name "presentation-select-revision-write"
        $path = Join-Path $fixture "crates\desktop\src\presentation_style.rs"
        $text = [System.IO.File]::ReadAllText($path)
        $newline = if ($text.Contains("`r`n")) { "`r`n" } else { "`n" }
        $original = '        self.selection = selection;' + $newline +
            '        self.revision = revision;'
        ([regex]::Matches($text, [regex]::Escape($original))).Count | Should -Be 1
        [System.IO.File]::WriteAllText(
            $path,
            $text.Replace($original, '        self.selection = selection;')
        )

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-REVISION*"
    }

    It "rejects a private UiPalette property in another component" {
        $fixture = New-DesktopAuditFixture -Name "skin-private-palette-property"
        $path = Join-Path $fixture "crates\desktop\ui\views\activity-view.slint"
        $text = [System.IO.File]::ReadAllText($path)
        $text = $text.Replace(
            'import { UiTokens } from "../tokens.slint";',
            'import { UiPalette, UiTokens } from "../tokens.slint";'
        )
        $original = 'export component ActivityView inherits Rectangle {'
        $replacement = $original + [Environment]::NewLine +
            '    private property <UiPalette> alternate-palette: UiTokens.palette;'
        ([regex]::Matches($text, [regex]::Escape($original))).Count | Should -Be 1
        [System.IO.File]::WriteAllText($path, $text.Replace($original, $replacement))

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PRESENTATION-OWNER*"
    }

    It "rejects a presentation palette callback in another component" {
        $fixture = New-DesktopAuditFixture -Name "skin-palette-callback"
        $path = Join-Path $fixture "crates\desktop\ui\views\activity-view.slint"
        $text = [System.IO.File]::ReadAllText($path)
        $original = 'export component ActivityView inherits Rectangle {'
        $replacement = $original + [Environment]::NewLine +
            '    callback select-presentation-palette(int);'
        ([regex]::Matches($text, [regex]::Escape($original))).Count | Should -Be 1
        [System.IO.File]::WriteAllText($path, $text.Replace($original, $replacement))

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PRESENTATION-OWNER*"
    }

    It "rejects a dead ten-thousand-loop decoy around a weakened live UI proof" {
        $fixture = New-DesktopAuditFixture -Name "skin-ui-dead-switch-decoy"
        $path = Join-Path $fixture "crates\desktop\tests\presentation_skin_ui_contract.rs"
        $text = [System.IO.File]::ReadAllText($path)
        $original = '    for index in 0..10_000 {'
        $newline = if ($text.Contains("`r`n")) { "`r`n" } else { "`n" }
        $replacement = '    if false {' + $newline +
            '        for index in 0..10_000 { let _ = index; }' + $newline +
            '    }' + $newline + $newline +
            '    for index in 0..1_000 {'
        ([regex]::Matches($text, [regex]::Escape($original))).Count | Should -Be 1
        [System.IO.File]::WriteAllText($path, $text.Replace($original, $replacement))

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DENSITY-STRESS*"
    }
}
