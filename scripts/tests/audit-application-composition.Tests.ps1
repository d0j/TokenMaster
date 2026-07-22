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
            Copy-Item -LiteralPath (Join-Path $RepositoryRoot "crates\runtime") `
                -Destination $crateParent -Recurse
            Copy-Item -LiteralPath (Join-Path $RepositoryRoot "crates\platform") `
                -Destination $crateParent -Recurse
            Copy-Item -LiteralPath (Join-Path $RepositoryRoot "crates\state") `
                -Destination $crateParent -Recurse
            return $fixture
        }
    }

    It "accepts the current allowlisted ExitCode composition" {
        $fixture = New-AppAuditFixture -Name "current-composition"

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Not -Throw
    }

    It "rejects removing the typed history range router" {
        $fixture = New-AppAuditFixture -Name "history-range-router"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'DesktopHistoryRangeIntentRouter::new()',
            'DesktopHistoryRangeIntentRouter::unbound()'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-HISTORY-RANGE-SINK*"
    }

    It "rejects querying from the application history range route" {
        $fixture = New-AppAuditFixture -Name "history-range-query-route"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [regex]::Replace(
            [System.IO.File]::ReadAllText($path),
            '(?s)(impl ApplicationHistoryRangeIntentSink \{.*?fn request\(&self, intent: DesktopHistoryRangeIntent\).*?)bundle\s*\.\s*controller\s*\.\s*request_history_range\(intent\)',
            '${1}source.usage_analytics(request)',
            1
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-HISTORY-RANGE-SINK*"
    }

    It "rejects removing the history range terminal notifier attachment" {
        $fixture = New-AppAuditFixture -Name "history-range-terminal-attachment"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '.attach_terminal_history_range_notifier(live_bridge.terminal_history_range_notifier())',
            '.attach_removed_history_range_notifier(live_bridge.terminal_history_range_notifier())'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-HISTORY-RANGE-TERMINAL*"
    }

    It "rejects a duplicate history range terminal notifier attachment" {
        $fixture = New-AppAuditFixture -Name "history-range-terminal-duplicate"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '.attach_terminal_history_range_notifier(live_bridge.terminal_history_range_notifier())',
            ".attach_terminal_history_range_notifier(live_bridge.terminal_history_range_notifier())`r`n        .attach_terminal_history_range_notifier(live_bridge.terminal_history_range_notifier())"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-HISTORY-RANGE-TERMINAL*"
    }

    It "rejects a helper-located second history range terminal notifier attachment" {
        $fixture = New-AppAuditFixture -Name "history-range-terminal-helper-duplicate"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path) + @'

fn attach_history_range_terminal_decoy(controller: &mut DesktopController, live_bridge: &DesktopBridge) {
    let _ = controller.attach_terminal_history_range_notifier(live_bridge.terminal_history_range_notifier());
}
'@
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-HISTORY-RANGE-TERMINAL*"
    }

    It "rejects removing the typed session-detail router" {
        $fixture = New-AppAuditFixture -Name "session-detail-router"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'DesktopSessionDetailIntentRouter::new()',
            'DesktopSessionDetailIntentRouter::unbound()'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-SESSION-DETAIL-ROUTER*"
    }

    It "rejects routing session detail outside the current bundle controller" {
        $fixture = New-AppAuditFixture -Name "session-detail-current-bundle"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'bundle.controller.request_session_detail(intent)',
            'obsolete_controller.request_session_detail(intent)'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-SESSION-DETAIL-CURRENT-BUNDLE*"
    }

    It "rejects fabricating session-detail admission in safe mode" {
        $fixture = New-AppAuditFixture -Name "session-detail-safe-mode"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'let Some(bundle) = slot.as_ref() else {',
            'let Some(bundle) = slot.as_ref().or_else(fake_bundle) else {'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-SESSION-DETAIL-SAFE-MODE*"
    }

    It "rejects blocking the UI thread on current-bundle ownership" {
        $fixture = New-AppAuditFixture -Name "session-detail-blocking-lock"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'let Ok(slot) = bundle.try_lock() else {',
            'let Ok(slot) = bundle.lock() else {'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-SESSION-DETAIL-NONBLOCKING*"
    }

    It "rejects a strong current-bundle Sessions page sink" {
        $fixture = New-AppAuditFixture -Name "session-page-strong-bundle"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [regex]::Replace(
            [System.IO.File]::ReadAllText($path),
            'struct ApplicationSessionPageIntentSink\s*\{\s*bundle:\s*Weak<Mutex<ApplicationBundleSlot>>',
            'struct ApplicationSessionPageIntentSink { bundle: Arc<Mutex<ApplicationBundleSlot>>',
            1
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-SESSION-PAGE-SINK*"
    }

    It "rejects blocking Sessions page routing on the current bundle" {
        $fixture = New-AppAuditFixture -Name "session-page-blocking-lock"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [regex]::Replace(
            [System.IO.File]::ReadAllText($path),
            '(?s)(impl ApplicationSessionPageIntentSink \{.*?let slot = )bundle\.try_lock\(\)\.map_err\(\|_\| \(\)\)\?;',
            '${1}bundle.lock().map_err(|_| ())?;',
            1
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-SESSION-PAGE-SINK*"
    }

    It "rejects bypassing the typed Sessions page request dispatch" {
        $fixture = New-AppAuditFixture -Name "session-page-bypass-request"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [regex]::Replace(
            [System.IO.File]::ReadAllText($path),
            '(?s)(impl DesktopSessionPageIntentSink for ApplicationSessionPageIntentSink \{.*?fn submit\(&self, intent: DesktopSessionPageIntent\) -> DesktopSessionPageIntentAdmission \{\s*)match self\.request\(intent\) \{',
            '${1}match self.request_directly(intent) {',
            1
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-SESSION-PAGE-SINK*"
    }

    It "rejects fabricating accepted Sessions page admission" {
        $fixture = New-AppAuditFixture -Name "session-page-fabricated-admission"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'DesktopSessionPageIntentAdmission::Accepted,',
            'DesktopSessionPageIntentAdmission::Rejected,'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-SESSION-PAGE-SINK*"
    }

    It "rejects removing terminal Sessions recovery from the live bridge" {
        $fixture = New-AppAuditFixture -Name "session-page-terminal-recovery"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $text = $original.Replace(
            '.attach_terminal_navigation_notifier(live_bridge.terminal_navigation_notifier())',
            '.skip_terminal_navigation_notifier(live_bridge.terminal_navigation_notifier())'
        )
        $text | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-SESSION-PAGE-TERMINAL-RECOVERY*"
    }

    It "rejects a comment-only terminal Sessions recovery anchor" {
        $fixture = New-AppAuditFixture -Name "session-page-terminal-comment-anchor"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $text = $original.Replace(
            '.attach_terminal_navigation_notifier(live_bridge.terminal_navigation_notifier())',
            ".skip_terminal_navigation_notifier(live_bridge.terminal_navigation_notifier())`r`n        // controller.attach_terminal_navigation_notifier(live_bridge.terminal_navigation_notifier())"
        )
        $text | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-SESSION-PAGE-TERMINAL-RECOVERY*"
    }

    It "rejects a nested-comment-only terminal Sessions recovery anchor" {
        $fixture = New-AppAuditFixture -Name "session-page-terminal-nested-comment-anchor"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $text = $original.Replace(
            '.attach_terminal_navigation_notifier(live_bridge.terminal_navigation_notifier())',
            ".skip_terminal_navigation_notifier(live_bridge.terminal_navigation_notifier())`r`n        /* outer /* inner */ controller.attach_terminal_navigation_notifier(live_bridge.terminal_navigation_notifier()) */"
        )
        $text | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-SESSION-PAGE-TERMINAL-RECOVERY*"
    }

    It "rejects a cfg-test-only terminal Sessions recovery attachment" {
        $fixture = New-AppAuditFixture -Name "session-page-terminal-cfg-test-anchor"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $text = $original.Replace(
            '.attach_terminal_navigation_notifier(live_bridge.terminal_navigation_notifier())',
            ".skip_terminal_navigation_notifier(live_bridge.terminal_navigation_notifier())`r`n        #[cfg(test)]`r`n        { controller.attach_terminal_navigation_notifier(live_bridge.terminal_navigation_notifier()); }"
        )
        $text | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-SESSION-PAGE-TERMINAL-RECOVERY*"
    }

    It "rejects the complete terminal Sessions sequence inside a cfg-test block" {
        $fixture = New-AppAuditFixture -Name "session-page-terminal-cfg-test-sequence"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $original = [System.IO.File]::ReadAllText($path)
        $pattern = 'controller\s*\.attach_snapshot_notifier\(live_bridge\.notifier\(\)\)\s*\.map_err\(\|_\| ApplicationError::controller\(\)\)\?;\s*controller\s*\.attach_terminal_navigation_notifier\(live_bridge\.terminal_navigation_notifier\(\)\)\s*\.map_err\(\|_\| ApplicationError::controller\(\)\)\?;\s*controller\s*\.attach_terminal_history_range_notifier\(live_bridge\.terminal_history_range_notifier\(\)\)\s*\.map_err\(\|_\| ApplicationError::controller\(\)\)\?;\s*let refresh_ingress = controller\.refresh_ingress\(\);'
        $replacement = @'
#[cfg(test)]
    {
        controller
            .attach_snapshot_notifier(live_bridge.notifier())
            .map_err(|_| ApplicationError::controller())?;
        controller
            .attach_terminal_navigation_notifier(live_bridge.terminal_navigation_notifier())
            .map_err(|_| ApplicationError::controller())?;
        controller
            .attach_terminal_history_range_notifier(live_bridge.terminal_history_range_notifier())
            .map_err(|_| ApplicationError::controller())?;
        let refresh_ingress = controller.refresh_ingress();
        drop(refresh_ingress);
    }
    let refresh_ingress = controller.refresh_ingress();
'@
        $text = [regex]::Replace($original, $pattern, $replacement, 1)
        $text | Should -Not -Be $original
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-SESSION-PAGE-TERMINAL-RECOVERY*"
    }

    It "reports one executable terminal Sessions recovery attachment" {
        $fixture = New-AppAuditFixture -Name "session-page-terminal-receipt"

        $receipt = (& $Audit -RepositoryRoot $fixture -SourceOnly) | ConvertFrom-Json
        $receipt.session_page_terminal_attachment_count | Should -Be 1
    }

    It "rejects removing the production lifecycle router" {
        $fixture = New-AppAuditFixture -Name "lifecycle-router"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'DesktopLifecycleIntentRouter::new()',
            'DesktopLifecycleIntentRouter::unbound()'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-LIFECYCLE-ROUTER*"
    }

    It "rejects routing the tray compact action to another route" {
        $fixture = New-AppAuditFixture -Name "lifecycle-compact-route"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'DesktopLifecycleIntent::OpenCompact => Self::OpenRoute("compact_widget")',
            'DesktopLifecycleIntent::OpenCompact => Self::OpenRoute("settings")'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-LIFECYCLE-COMPACT*"
    }

    It "rejects showing the optional tray before the visible fallback" {
        $fixture = New-AppAuditFixture -Name "lifecycle-visible-fallback"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'let _ = self.shell.show_lifecycle_surface();',
            'let _ = show_lifecycle_surface_before_window();'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-LIFECYCLE-SURFACE*"
    }

    It "rejects showing a tray route without foreground activation" {
        $fixture = New-AppAuditFixture -Name "lifecycle-focus"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'tokenmaster_desktop::activate_window(window.window())',
            'Ok(())'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-LIFECYCLE-FOCUS*"
    }

    It "rejects removing the early current-session claim" {
        $fixture = New-AppAuditFixture -Name "current-session-early"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'CurrentSessionIntegration::claim()',
            'claim_after_application_start()'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-CURRENT-SESSION-EARLY*"
    }

    It "rejects replacing the stable current-session failure" {
        $fixture = New-AppAuditFixture -Name "current-session-error"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'ApplicationError::current_session_unavailable()',
            'ApplicationError::internal()'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-CURRENT-SESSION-ERROR*"
    }

    It "rejects current-session activation identifier drift" {
        $fixture = New-AppAuditFixture -Name "current-session-identifier"
        $path = Join-Path $fixture "crates\platform\src\current_session.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'Local\\TokenMaster.CurrentSession.Activation.v1',
            'Global\\TokenMaster.CurrentSession.Activation.v1'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-CURRENT-SESSION-OWNER*"
    }

    It "rejects current-session hotkey drift" {
        $fixture = New-AppAuditFixture -Name "current-session-hotkey"
        $path = Join-Path $fixture "crates\platform\src\current_session.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'const VIRTUAL_KEY_T: u32 = 0x54;',
            'const VIRTUAL_KEY_T: u32 = 0x44;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-CURRENT-SESSION-HOTKEY*"
    }

    It "rejects dropping current-session join before clean publication" {
        $fixture = New-AppAuditFixture -Name "current-session-shutdown"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [regex]::Replace(
            [System.IO.File]::ReadAllText($path),
            'current_session\s*\.\s*shutdown\(\)',
            'current_session.detach()',
            1
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-CURRENT-SESSION-SHUTDOWN*"
    }

    It "rejects unbounded current-session pending activation drift" {
        $fixture = New-AppAuditFixture -Name "current-session-pending-capacity"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'self.pending.swap(true, Ordering::AcqRel)',
            'self.pending.swap(false, Ordering::AcqRel)'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-CURRENT-SESSION-CAPACITY*"
    }

    It "rejects a strong current-session activation ownership cycle" {
        $fixture = New-AppAuditFixture -Name "current-session-strong-cycle"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'self_weak: Weak<Self>',
            'self_strong: Arc<Self>'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-CURRENT-SESSION-CAPACITY*"
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
            -Value 'fn duplicate_operation_worker() { let _ = ApplicationOperationWorker::spawn_with_payload('

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
            'mut target: SelectedOutputFile',
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
            'execute_manual_backup_command(bundle, reliable_state, permit)',
            'execute_unbound_backup(bundle, permit)'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-BACKUP-COMMAND*"
    }

    It "rejects publishing running state only at admission instead of actual execution" {
        $fixture = New-AppAuditFixture -Name "operation-actual-start-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '.publish_operation(Some(application_operation_running(permit.command())))',
            '.publish_operation(Some(application_operation_completion(permit.command(), ApplicationCommandExecution::Succeeded)))'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-OPERATION-ACTUAL-START*"
    }

    It "rejects removing the manual backup atomic promotion projection" {
        $fixture = New-AppAuditFixture -Name "manual-backup-atomic-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'publish_atomic_operation(reliable_state, permit.command());',
            'publish_manual_backup_state(reliable_state, permit.command());'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-MANUAL-BACKUP-ATOMIC*"
    }

    It "rejects removing the presentation density atomic projection" {
        $fixture = New-AppAuditFixture -Name "presentation-density-atomic-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '|| publish_atomic_operation(reliable_state, permit.command()),',
            '|| publish_presentation_state(reliable_state, permit.command()),'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-PRESENTATION-ATOMIC*"
    }

    It "rejects losing the durable source-reconciliation obligation after reconstruction" {
        $fixture = New-AppAuditFixture -Name "rebuild-durable-reconcile-drift"
        $path = Join-Path $fixture "crates\app\src\state.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'journal.phase() == RecoveryPhase::Complete && journal.backup().is_none()',
            'journal.phase() == RecoveryPhase::Complete && journal.backup().is_some()'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-REBUILD-DURABLE-RECONCILE*"
    }

    It "rejects bypassing cold-start reconstruction reconciliation" {
        $fixture = New-AppAuditFixture -Name "rebuild-cold-reconcile-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'if preflight.requires_source_reconciliation() {',
            'if false {'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-REBUILD-COLD-RECONCILE*"
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
            Should -Throw "*TM-APP-RECOVERY-LAUNCH*"
    }

    It "rejects removing the no-backup rebuild binding" {
        $fixture = New-AppAuditFixture -Name "rebuild-binding-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '(ApplicationCommand::Rebuild, ApplicationOperationPayload::Empty)',
            '(ApplicationCommand::Rebuild, ApplicationOperationPayload::ConfigInput(_))'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-REBUILD-BINDING*"
    }

    It "rejects weakening authoritative recovery reconciliation" {
        $fixture = New-AppAuditFixture -Name "rebuild-reconcile-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '.refresh_now(RefreshUrgency::Recovery)',
            '.refresh_now(RefreshUrgency::Hint)'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-REBUILD-RECONCILE*"
    }

    It "rejects reporting rebuild success before reconciliation completes" {
        $fixture = New-AppAuditFixture -Name "rebuild-reconcile-wait-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'wait_for_reconstructed_reconciliation(&started.live)',
            'skip_reconstructed_reconciliation(&started.live)'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-REBUILD-RECONCILE-WAIT*"
    }

    It "rejects binding the recovery receipt after restored lifecycle work" {
        $fixture = New-AppAuditFixture -Name "restore-receipt-order-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path)
        $needle = '.bind_recovery_launch(receipt)?;'
        $binding = $text.IndexOf($needle, [System.StringComparison]::Ordinal)
        $binding | Should -BeGreaterOrEqual 0
        $text = $text.Remove($binding, $needle.Length)
        $text += "`nfn bind_too_late() { preflight.bind_recovery_launch(receipt)?; }`n"
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
        $text = [regex]::Replace(
            [System.IO.File]::ReadAllText($path),
            'struct ApplicationRuntimeNotifier\s*\{\s*bundle:\s*Weak<Mutex<ApplicationBundleSlot>>',
            'struct ApplicationRuntimeNotifier { bundle: Arc<Mutex<ApplicationBundleSlot>>',
            1
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

    It "rejects direct SQL authority without confusing policy update identifiers" {
        $fixture = New-AppAuditFixture -Name "forbidden-sql-authority"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\application.rs") `
            -Value 'const PRIVATE_SQL: &str = "UPDATE settings SET value = 1";'

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

    It "rejects loss of the dedicated recovery adversarial gate" {
        $fixture = New-AppAuditFixture -Name "missing-recovery-adversarial"
        $path = Join-Path $fixture "crates\app\tests\recovery_adversarial_contract.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'application_gate_is_bound_to_the_complete_state_recovery_matrix',
            'coverage_removed'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-RECOVERY-ADVERSARIAL*"
    }

    It "rejects replacing executable recovery coverage with source-only anchors" {
        $fixture = New-AppAuditFixture -Name "missing-executable-recovery-module"
        $path = Join-Path $fixture "crates\app\tests\recovery_adversarial_contract.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'mod restore_contract;',
            'mod removed_restore_contract;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-RECOVERY-ADVERSARIAL*"
    }

    It "rejects notification acknowledgement retry drift" {
        $fixture = New-AppAuditFixture -Name "notification-retry-drift"
        $path = Join-Path $fixture "crates\app\src\notification.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'NOTIFICATION_ACK_RETRY: Duration = Duration::from_secs(60)',
            'NOTIFICATION_ACK_RETRY: Duration = Duration::from_secs(30)'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-NOTIFICATION-RETRY*"
    }

    It "rejects a second notification receipt worker" {
        $fixture = New-AppAuditFixture -Name "notification-second-worker"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\notification.rs") `
            -Value 'fn duplicate_receipt_worker() { let _ = thread::Builder::new(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-NOTIFICATION-WORKER*"
    }

    It "rejects clearing local backpressure before runtime release" {
        $fixture = New-AppAuditFixture -Name "notification-release-order"
        $path = Join-Path $fixture "crates\app\src\notification.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'if !port.release()? {',
            "signal.clear_in_flight();`r`n    if !port.release()? {"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-NOTIFICATION-RELEASE-ORDER*"
    }

    It "rejects removing terminal presentation failure release" {
        $fixture = New-AppAuditFixture -Name "notification-terminal-release"
        $path = Join-Path $fixture "crates\app\src\notification.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'ReceiptAction::Failed => {',
            'ReceiptAction::Failed => acknowledge_presented {'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-NOTIFICATION-RELEASE*"
    }

    It "rejects treating a false runtime release as confirmed" {
        $fixture = New-AppAuditFixture -Name "notification-false-release"
        $path = Join-Path $fixture "crates\app\src\notification.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'if !port.release()? {',
            'if false {'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-NOTIFICATION-RELEASE-ORDER*"
    }

    It "rejects removing the bounded presentation re-pump" {
        $fixture = New-AppAuditFixture -Name "notification-repump"
        $path = Join-Path $fixture "crates\app\src\notification.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'let _ = pump_presentation(signal, port, presenter.as_ref());',
            'let _ = presenter;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-NOTIFICATION-REPUMP*"
    }

    It "rejects retrying terminal acknowledgement through re-presentation" {
        $fixture = New-AppAuditFixture -Name "notification-terminal-ack-repump"
        $path = Join-Path $fixture "crates\app\src\notification.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'let _ = release_with_retry(signal, port, retry);',
            'release_then_retry_presentation(signal, port, presenter, retry);'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-NOTIFICATION-TERMINAL-ACK*"
    }

    It "rejects making initial notification presentation depend on controller success" {
        $fixture = New-AppAuditFixture -Name "notification-pump-order"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path)
        $pump = @'
        if let Some(presentation) = self.notification_presentation.as_ref() {
            let _ = presentation.pump();
        }
'@
        $text = $text.Replace($pump, '')
        $refresh = @'
        self.controller
            .refresh(DesktopRefreshUrgency::Hint)
            .map_err(|_| ApplicationError::controller())?;
'@
        $text = $text.Replace($refresh, $refresh + $pump)
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-NOTIFICATION-PUMP-ORDER*"
    }

    It "rejects removing runtime acknowledgement panic rollback" {
        $fixture = New-AppAuditFixture -Name "notification-panic-rollback"
        $path = Join-Path $fixture "crates\runtime\src\reminder\runtime.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'std::panic::catch_unwind',
            'removed_panic_boundary'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-NOTIFICATION-PANIC-ROLLBACK*"
    }

    It "rejects removing outer runtime mutex poison release recovery" {
        $fixture = New-AppAuditFixture -Name "notification-poison-release"
        $path = Join-Path $fixture "crates\app\src\notification.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '.unwrap_or_else(std::sync::PoisonError::into_inner)',
            '.map_err(|_| PresentationFailure::Internal)?'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-NOTIFICATION-POISON-RELEASE*"
    }

    It "rejects acknowledging reminders outside the dedicated presentation port" {
        $fixture = New-AppAuditFixture -Name "notification-ack-authority"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\application.rs") `
            -Value 'fn false_route_ack(runtime: &BenefitReminderRuntime) { let _ = runtime.acknowledge_notifications(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-NOTIFICATION-ACK-AUTHORITY*"
    }

    It "rejects joining the notification worker after reminder shutdown" {
        $fixture = New-AppAuditFixture -Name "notification-shutdown-order"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path)
        $presentation = 'if let Some(mut presentation) = self.notification_presentation.take()'
        $start = $text.IndexOf($presentation, [System.StringComparison]::Ordinal)
        $start | Should -BeGreaterOrEqual 0
        $end = $text.IndexOf('        if self.maintenance.pause()', $start, [System.StringComparison]::Ordinal)
        $block = $text.Substring($start, $end - $start)
        $text = $text.Remove($start, $end - $start)
        $shutdown = 'remember_failure(&mut first, reminder.shutdown().map(|_| ())),'
        $insert = $text.IndexOf($shutdown, [System.StringComparison]::Ordinal)
        $text = $text.Insert($insert + $shutdown.Length, "`r`n        $block")
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-NOTIFICATION-SHUTDOWN-ORDER*"
    }

    It "rejects an unsealed reminder operation payload" {
        $fixture = New-AppAuditFixture -Name "reminder-unsealed-payload"
        $path = Join-Path $fixture "crates\app\src\command.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'payload: ApplicationOperationPayload::ReminderPolicy(update),',
            'payload: ApplicationOperationPayload::Empty,'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-REMINDER-SEALED*"
    }

    It "rejects dropping the replaceable latest reminder payload" {
        $fixture = New-AppAuditFixture -Name "reminder-latest-wins-drift"
        $path = Join-Path $fixture "crates\app\src\operation.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'pending.payload = payload;',
            'let _ = payload;'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-REMINDER-LATEST-WINS*"
    }

    It "rejects bypassing visible Pending for a reminder save" {
        $fixture = New-AppAuditFixture -Name "reminder-visible-pending-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'publish_pending_reminder_policy(reliable_state, permit.command(), &pending_policy)',
            'publish_pending_reminder_operation(reliable_state, permit.command())'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-REMINDER-VISIBLE-PENDING*"
    }

    It "rejects reminder profile generation drift" {
        $fixture = New-AppAuditFixture -Name "reminder-generation-drift"
        $path = Join-Path $fixture "crates\app\src\state.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '.checked_add(1)',
            '.checked_add(2)'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-REMINDER-GENERATION*"
    }

    It "rejects synchronizing reminder state before durable settings save" {
        $fixture = New-AppAuditFixture -Name "reminder-settings-first-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'Ok(()) => execute_state_command(synchronize_reminder_policy_after_settings(',
            'Ok(()) => execute_state_command(synchronize_reminder_policy_before_settings('
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-REMINDER-SETTINGS-FIRST*"
    }

    It "rejects publishing synchronized before the global profile commit" {
        $fixture = New-AppAuditFixture -Name "reminder-sync-state-order"
        $path = Join-Path $fixture "crates\app\src\state.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '.store(REMINDER_SYNC_SYNCHRONIZED, Ordering::Release);',
            '.store(REMINDER_SYNC_PENDING, Ordering::Release);'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-REMINDER-SYNC-STATE*"
    }

    It "rejects dropping reminder synchronization after confirmed config import" {
        $fixture = New-AppAuditFixture -Name "reminder-import-sync-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'Ok(_) => execute_state_command(synchronize_reminder_policy_after_settings(',
            'Ok(_) => execute_state_command(skip_reminder_policy_synchronization('
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-REMINDER-IMPORT-BINDING*"
    }

    It "rejects overwriting retryable Pending after startup reminder contention" {
        $fixture = New-AppAuditFixture -Name "reminder-startup-pending-drift"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'Err(_) => OptionalReminderRuntime::failed(RuntimeErrorCode::StoreUnavailable),',
            'Err(_) => { state.mark_reminder_unavailable(); OptionalReminderRuntime::failed(RuntimeErrorCode::StoreUnavailable) },'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-REMINDER-STARTUP-PENDING*"
    }

    It "reports the bounded notification composition receipt" {
        $fixture = New-AppAuditFixture -Name "notification-receipt"
        $receipt = & $Audit -RepositoryRoot $fixture -SourceOnly | ConvertFrom-Json

        $receipt.rust_source_file_count | Should -Be 8
        $receipt.notification_receipt_worker_count | Should -Be 1
        $receipt.notification_ack_retry_seconds | Should -Be 60
        $receipt.notification_presentation_coordinator_count | Should -Be 1
        $receipt.notification_runtime_ack_authority_count | Should -Be 1
        $receipt.notification_confirmed_release_count | Should -Be 1
        $receipt.notification_bounded_repump_count | Should -Be 1
        $receipt.notification_runtime_panic_rollback_count | Should -Be 1
        $receipt.reminder_startup_pending_binding_count | Should -Be 1
        $receipt.lifecycle_router_count | Should -Be 1
        $receipt.lifecycle_intent_count | Should -Be 5
        $receipt.lifecycle_window_owner_count | Should -Be 1
        $receipt.lifecycle_polling_surface_count | Should -Be 0
        $receipt.current_session_claim_count | Should -Be 1
        $receipt.current_session_owner_count | Should -Be 1
        $receipt.current_session_thread_count | Should -Be 1
        $receipt.current_session_event_count | Should -Be 1
        $receipt.current_session_hotkey_count | Should -Be 1
        $receipt.current_session_polling_surface_count | Should -Be 0
        $receipt.current_session_bridge_count | Should -Be 1
        $receipt.current_session_pending_bit_count | Should -Be 1
        $receipt.current_session_scheduled_bit_count | Should -Be 1
        $receipt.current_session_scheduled_task_count | Should -Be 1
    }

    It "rejects a partial density-only presentation payload" {
        $fixture = New-AppAuditFixture -Name "presentation-partial-density"
        $path = Join-Path $fixture "crates\app\src\command.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'ApplicationPresentationUpdate::new(',
            'ApplicationPresentationDensityUpdate::new('
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-PRESENTATION-COMPLETE*"
    }

    It "rejects a partial skin-only presentation payload" {
        $fixture = New-AppAuditFixture -Name "presentation-partial-skin"
        $path = Join-Path $fixture "crates\\app\\src\\command.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'selection: DesktopPresentationSelection,',
            'skin: tokenmaster_desktop::DesktopSkin,'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-PRESENTATION-COMPLETE*"
    }

    It "rejects a schema range wider than v1 through v3" {
        $fixture = New-AppAuditFixture -Name "presentation-schema-range"
        $path = Join-Path $fixture "crates\state\src\settings\migration.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'SETTINGS_SCHEMA_VERSION => decode_portable_v3(bytes),',
            '3 | 4 => decode_portable_v3(bytes),'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-PRESENTATION-SCHEMA*"
    }

    It "rejects a missing v2 Refined migration default" {
        $fixture = New-AppAuditFixture -Name "presentation-v2-default"
        $path = Join-Path $fixture "crates\state\src\settings\migration.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'PresentationSkin::Refined',
            'PresentationSkin::Graphite'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-PRESENTATION-SCHEMA*"
    }

    It "rejects a second presentation worker authority" {
        $fixture = New-AppAuditFixture -Name "presentation-second-worker"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\operation.rs") `
            -Value 'fn skin_worker() { std::thread::spawn(|| {}); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-OPERATION-SPAWN*"
    }

    It "rejects a generic second operation channel bypass" {
        $fixture = New-AppAuditFixture -Name "presentation-second-generic-channel"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\operation.rs") `
            -Value 'fn presentation_channel() { let _ = std::sync::mpsc::sync_channel::<u8>(2); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-OPERATION-SPAWN*"
    }

    It "rejects a presentation conversion that loses the skin axis" {
        $fixture = New-AppAuditFixture -Name "presentation-constant-skin"
        $path = Join-Path $fixture "crates\app\src\command.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'match (self.selection.density(), self.selection.skin()) {',
            'match (self.selection.density(), tokenmaster_desktop::DesktopSkin::Refined) {'
        )
        [System.IO.File]::WriteAllText($path, $text)
        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-PRESENTATION-COMPLETE*"
    }

    It "rejects one swapped output in the exact nine-pair presentation conversion" {
        $fixture = New-AppAuditFixture -Name "presentation-swapped-output-pair"
        $path = Join-Path $fixture "crates\app\src\command.rs"
        $text = [System.IO.File]::ReadAllText($path)
        $newline = if ($text.Contains("`r`n")) { "`r`n" } else { "`n" }
        $original = '                tokenmaster_state::PresentationDensity::Comfortable,' + $newline +
            '                tokenmaster_state::PresentationSkin::Graphite,'
        $replacement = '                tokenmaster_state::PresentationDensity::Comfortable,' + $newline +
            '                tokenmaster_state::PresentationSkin::Ember,'
        ([regex]::Matches($text, [regex]::Escape($original))).Count | Should -Be 1
        [System.IO.File]::WriteAllText($path, $text.Replace($original, $replacement))

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-PRESENTATION-COMPLETE*"
    }

    It "rejects an unbounded second operation channel" {
        $fixture = New-AppAuditFixture -Name "presentation-second-unbounded-channel"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\operation.rs") `
            -Value 'fn presentation_unbounded_channel() { let _ = std::sync::mpsc::channel::<u8>(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-OPERATION-SPAWN*"
    }

    It "rejects a second operation channel hidden behind an import alias" {
        $fixture = New-AppAuditFixture -Name "presentation-aliased-channel"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\operation.rs") `
            -Value 'use std::sync::mpsc::channel as open_channel; fn aliased_channel() { let _ = open_channel::<u8>(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-OPERATION-SPAWN*"
    }
}
