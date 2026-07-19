$ErrorActionPreference = 'Stop'
Describe 'current-user startup source audit' {
BeforeAll {
$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Audit = Join-Path $RepositoryRoot 'scripts\audit-current-user-startup.ps1'

function New-StartupAuditFixture([string]$Name) {
    $root = Join-Path $TestDrive $Name
    $files = @(
        'Cargo.toml',
        'crates/platform/src/current_user_startup.rs',
        'crates/platform/src/lib.rs',
        'crates/app/src/application.rs',
        'crates/desktop/src/reliable_state.rs',
        'crates/desktop/src/ui.rs',
        'crates/desktop/ui/main.slint',
        'crates/desktop/ui/views/settings-view.slint',
        'crates/state/src/package/reader.rs',
        'crates/state/src/package/writer.rs',
        'crates/app/src/state.rs',
        'crates/app/src/command.rs'
    )
    foreach ($relative in $files) {
        $target = Join-Path $root $relative
        New-Item -ItemType Directory -Force -Path (Split-Path $target -Parent) | Out-Null
        Copy-Item -LiteralPath (Join-Path $RepositoryRoot $relative) -Destination $target
    }
    $settingsTarget = Join-Path $root 'crates/state/src/settings'
    New-Item -ItemType Directory -Force -Path $settingsTarget | Out-Null
    Get-ChildItem -LiteralPath (Join-Path $RepositoryRoot 'crates/state/src/settings') -Filter '*.rs' -File |
        Copy-Item -Destination $settingsTarget
    return $root
}

function Replace-StartupFixtureText(
    [string]$Root,
    [string]$RelativePath,
    [string]$Old,
    [string]$New
) {
    $path = Join-Path $Root $RelativePath
    $text = [System.IO.File]::ReadAllText($path)
    if (-not $text.Contains($Old)) {
        throw "fixture anchor missing: $RelativePath :: $Old"
    }
    [System.IO.File]::WriteAllText($path, $text.Replace($Old, $New))
}

}

    It 'accepts the exact bounded current-user startup contour' {
        $receipt = & $Audit -RepositoryRoot $RepositoryRoot | ConvertFrom-Json
        $receipt.result | Should -Be 'pass'
        $receipt.typed_intent_count | Should -Be 3
        $receipt.accessible_action_count | Should -Be 4
        $receipt.portable_startup_field_count | Should -Be 0
    }

    It 'rejects machine-wide registry authority' {
        $fixture = New-StartupAuditFixture 'hklm'
        Replace-StartupFixtureText $fixture 'crates/platform/src/current_user_startup.rs' 'HKEY_CURRENT_USER' 'HKEY_LOCAL_MACHINE'
        { & $Audit -RepositoryRoot $fixture } | Should -Throw '*TM-STARTUP-HKCU-ONLY*'
    }

    It 'rejects a caller-selected Run key' {
        $fixture = New-StartupAuditFixture 'run-key'
        Replace-StartupFixtureText $fixture 'crates/platform/src/current_user_startup.rs' 'Software\\Microsoft\\Windows\\CurrentVersion\\Run' 'Software\\TokenMaster\\Run'
        { & $Audit -RepositoryRoot $fixture } | Should -Throw '*TM-STARTUP-FIXED-RUN-KEY*'
    }

    It 'rejects shell or process expansion' {
        $fixture = New-StartupAuditFixture 'process'
        Add-Content -LiteralPath (Join-Path $fixture 'crates/platform/src/current_user_startup.rs') -Value "`nuse std::process;"
        { & $Audit -RepositoryRoot $fixture } | Should -Throw '*TM-STARTUP-NO-PROCESS*'
    }

    It 'rejects losing the second readback call' {
        $fixture = New-StartupAuditFixture 'readback'
        Replace-StartupFixtureText $fixture 'crates/platform/src/current_user_startup.rs' "let read_result = unsafe {`n            RegGetValueW(" "let read_result = unsafe {`n            RegGetValueW_removed("
        { & $Audit -RepositoryRoot $fixture } | Should -Throw '*TM-STARTUP-READBACK*'
    }

    It 'rejects expanding the Windows Run command beyond 260 UTF-16 units' {
        $fixture = New-StartupAuditFixture 'command-limit'
        Replace-StartupFixtureText $fixture 'crates/platform/src/current_user_startup.rs' 'const MAX_COMMAND_UTF16_UNITS: usize = 260;' 'const MAX_COMMAND_UTF16_UNITS: usize = 261;'
        { & $Audit -RepositoryRoot $fixture } | Should -Throw '*TM-STARTUP-BOUNDED-COMMAND*'
    }

    It 'rejects deferring current-command validation until Enable' {
        $fixture = New-StartupAuditFixture 'late-command-validation'
        Replace-StartupFixtureText $fixture 'crates/platform/src/current_user_startup.rs' 'let command = build_command(&path)?;' 'let command = Vec::new();'
        { & $Audit -RepositoryRoot $fixture } | Should -Throw '*TM-STARTUP-EARLY-COMMAND-CAPABILITY*'
    }

    It 'rejects replacing same-handle no-follow opens with ordinary File open' {
        $fixture = New-StartupAuditFixture 'ordinary-open'
        Replace-StartupFixtureText $fixture 'crates/platform/src/current_user_startup.rs' 'crate::windows::open_regular_no_follow(path)' 'std::fs::File::open(path)'
        { & $Audit -RepositoryRoot $fixture } | Should -Throw '*TM-STARTUP-SAME-HANDLE-NO-FOLLOW*'
    }

    It 'rejects removing final handle-resolved local path proof' {
        $fixture = New-StartupAuditFixture 'final-handle-path'
        Replace-StartupFixtureText $fixture 'crates/platform/src/current_user_startup.rs' 'let resolved_path = resolved_local_path(&file)?;' 'let resolved_path = path.to_path_buf();'
        { & $Audit -RepositoryRoot $fixture } | Should -Throw '*TM-STARTUP-FINAL-HANDLE-CANONICAL*'
    }

    It 'rejects losing the extra malformed-REG-SZ NUL slot' {
        $fixture = New-StartupAuditFixture 'malformed-nul'
        Replace-StartupFixtureText $fixture 'crates/platform/src/current_user_startup.rs' '.checked_add(1)' '.checked_add(0)'
        { & $Audit -RepositoryRoot $fixture } | Should -Throw '*TM-STARTUP-MALFORMED-NUL-SLOT*'
    }

    It 'rejects removing the local-drive guard before registered-path I/O' {
        $fixture = New-StartupAuditFixture 'remote-path'
        Replace-StartupFixtureText $fixture 'crates/platform/src/current_user_startup.rs' 'if !supported_local_drive(&path) {' 'if false {'
        { & $Audit -RepositoryRoot $fixture } | Should -Throw '*TM-STARTUP-REMOTE-FAIL-CLOSED*'
    }

    It 'rejects removing current-executable reparse ancestry verification' {
        $fixture = New-StartupAuditFixture 'reparse-ancestry'
        Replace-StartupFixtureText $fixture 'crates/platform/src/current_user_startup.rs' 'reject_reparse_ancestry(path)?;' '/* ancestry verification removed */'
        { & $Audit -RepositoryRoot $fixture } | Should -Throw '*TM-STARTUP-REPARSE-ANCESTRY*'
    }

    It 'rejects removing executable physical identity verification' {
        $fixture = New-StartupAuditFixture 'identity'
        Replace-StartupFixtureText $fixture 'crates/platform/src/current_user_startup.rs' 'PhysicalFileIdentity::from_file(&file)' 'unverified_identity(&file)'
        { & $Audit -RepositoryRoot $fixture } | Should -Throw '*TM-STARTUP-PHYSICAL-IDENTITY*'
    }

    It 'rejects making stale enable implicit' {
        $fixture = New-StartupAuditFixture 'stale-enable'
        Replace-StartupFixtureText $fixture 'crates/platform/src/current_user_startup.rs' '(CurrentUserStartupAction::Enable, CurrentUserStartupStatus::StaleRelocation)' '(CurrentUserStartupAction::Enable, CurrentUserStartupStatus::Disabled)'
        { & $Audit -RepositoryRoot $fixture } | Should -Throw '*TM-STARTUP-EXPLICIT-STALE-ENABLE*'
    }

    It 'rejects startup state entering portable settings' {
        $fixture = New-StartupAuditFixture 'portable'
        Add-Content -LiteralPath (Join-Path $fixture 'crates/state/src/settings/value.rs') -Value "`nconst START_AT_LOGIN: bool = true;"
        { & $Audit -RepositoryRoot $fixture } | Should -Throw '*TM-STARTUP-PORTABLE-EXCLUSION*'
    }

    It 'rejects removing a typed Desktop action' {
        $fixture = New-StartupAuditFixture 'intent'
        Replace-StartupFixtureText $fixture 'crates/desktop/src/reliable_state.rs' '    RepairCurrentUserStartup,' '    RepairStartupLater,'
        { & $Audit -RepositoryRoot $fixture } | Should -Throw '*TM-STARTUP-DESKTOP-REPAIRCURRENTUSERSTARTUP*'
    }

    It 'rejects losing an explicit accessible action' {
        $fixture = New-StartupAuditFixture 'accessibility'
        Replace-StartupFixtureText $fixture 'crates/desktop/ui/views/settings-view.slint' 'accessible-label: "Repair TokenMaster startup registration";' 'accessible-label: "Repair";'
        { & $Audit -RepositoryRoot $fixture } | Should -Throw '*TM-STARTUP-ACCESSIBILITY*'
    }

    It 'rejects losing SettingsView action forwarding' {
        $fixture = New-StartupAuditFixture 'view-forwarding'
        Replace-StartupFixtureText $fixture 'crates/desktop/ui/views/settings-view.slint' 'clicked => { root.enable-current-user-startup(); }' 'clicked => { }'
        { & $Audit -RepositoryRoot $fixture } | Should -Throw '*TM-STARTUP-VIEW-FORWARD-ENABLE*'
    }

    It 'rejects losing MainWindow action forwarding' {
        $fixture = New-StartupAuditFixture 'main-forwarding'
        Replace-StartupFixtureText $fixture 'crates/desktop/ui/main.slint' 'repair-current-user-startup => { root.repair-current-user-startup(); }' 'repair-current-user-startup => { }'
        { & $Audit -RepositoryRoot $fixture } | Should -Throw '*TM-STARTUP-MAIN-FORWARD-REPAIR-CURRENT-USER-STARTUP*'
    }

    It 'rejects startup state entering the package codec' {
        $fixture = New-StartupAuditFixture 'package-portable'
        Add-Content -LiteralPath (Join-Path $fixture 'crates/state/src/package/writer.rs') -Value "`nconst START_AT_LOGIN: bool = true;"
        { & $Audit -RepositoryRoot $fixture } | Should -Throw '*TM-STARTUP-PORTABLE-EXCLUSION*'
    }
}
