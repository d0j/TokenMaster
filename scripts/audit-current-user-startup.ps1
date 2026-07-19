param(
    [Parameter(Mandatory = $true)]
    [string]$RepositoryRoot
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath $RepositoryRoot).Path

function Read-RequiredText([string]$RelativePath) {
    $path = Join-Path $root $RelativePath
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "TM-STARTUP-MISSING-SOURCE: $RelativePath"
    }
    return [System.IO.File]::ReadAllText($path)
}

function Count-Matches([string]$Text, [string]$Pattern) {
    return [regex]::Matches($Text, $Pattern).Count
}

function Require-Count(
    [string]$Code,
    [string]$Text,
    [string]$Pattern,
    [int]$Expected
) {
    $actual = Count-Matches $Text $Pattern
    if ($actual -ne $Expected) {
        throw "$Code`: expected $Expected, observed $actual"
    }
    return $actual
}

$cargo = Read-RequiredText 'Cargo.toml'
$platform = Read-RequiredText 'crates/platform/src/current_user_startup.rs'
$platformLib = Read-RequiredText 'crates/platform/src/lib.rs'
$application = Read-RequiredText 'crates/app/src/application.rs'
$desktopState = Read-RequiredText 'crates/desktop/src/reliable_state.rs'
$desktopUi = Read-RequiredText 'crates/desktop/src/ui.rs'
$mainUi = Read-RequiredText 'crates/desktop/ui/main.slint'
$settingsUi = Read-RequiredText 'crates/desktop/ui/views/settings-view.slint'

$registryFeatureCount = Require-Count 'TM-STARTUP-REGISTRY-FEATURE' $cargo '"Win32_System_Registry"' 1
$hkcuCount = Require-Count 'TM-STARTUP-HKCU-ONLY' $platform '\bHKEY_CURRENT_USER\b' 5
Require-Count 'TM-STARTUP-NO-HKLM' $platform '\bHKEY_LOCAL_MACHINE\b' 0 | Out-Null
Require-Count 'TM-STARTUP-FIXED-RUN-KEY' $platform 'w!\("Software\\\\Microsoft\\\\Windows\\\\CurrentVersion\\\\Run"\)' 1 | Out-Null
Require-Count 'TM-STARTUP-FIXED-VALUE' $platform 'w!\("TokenMaster"\)' 1 | Out-Null
$readbackCount = Require-Count 'TM-STARTUP-READBACK' $platform '\bRegGetValueW\(' 2
$writeCount = Require-Count 'TM-STARTUP-ONE-WRITE' $platform '\bRegSetKeyValueW\(' 1
$deleteCount = Require-Count 'TM-STARTUP-ONE-DELETE' $platform '\bRegDeleteKeyValueW\(' 1
Require-Count 'TM-STARTUP-REG-SZ' $platform '\bREG_SZ\.0\b' 1 | Out-Null
$commandLimitCount = Require-Count 'TM-STARTUP-BOUNDED-COMMAND' $platform 'const MAX_COMMAND_UTF16_UNITS: usize = 260;' 1
Require-Count 'TM-STARTUP-BOUNDED-REGISTRY-BYTES' $platform 'const MAX_REGISTRY_VALUE_BYTES: u32 = \(\(MAX_COMMAND_UTF16_UNITS \+ 1\) \* 2\) as u32;' 1 | Out-Null
Require-Count 'TM-STARTUP-EARLY-COMMAND-CAPABILITY' $platform 'let command = build_command\(&path\)\?;' 1 | Out-Null
$currentExecutableCount = Require-Count 'TM-STARTUP-CURRENT-EXE-ONLY' $platform 'std::env::current_exe\(\)' 1
$physicalIdentityCount = Require-Count 'TM-STARTUP-PHYSICAL-IDENTITY' $platform '\bPhysicalFileIdentity\b' 4
Require-Count 'TM-STARTUP-EXACT-QUOTED-PATH' $platform 'fn parse_exact_quoted_path\(' 1 | Out-Null
Require-Count 'TM-STARTUP-LOCAL-DRIVE-SYNTAX' $platform 'Prefix::Disk\(drive\) => drive' 1 | Out-Null
Require-Count 'TM-STARTUP-NO-DEVICE-PREFIX' $platform 'Prefix::VerbatimDisk' 0 | Out-Null
$driveTypeCount = Require-Count 'TM-STARTUP-LOCAL-DRIVE-TYPE' $platform '\bGetDriveTypeW\(' 1
Require-Count 'TM-STARTUP-REMOTE-FAIL-CLOSED' $platform 'if !supported_local_drive\(&path\) \{' 1 | Out-Null
Require-Count 'TM-STARTUP-REPARSE-ANCESTRY' $platform '\breject_reparse_ancestry\(path\)\?;' 1 | Out-Null
Require-Count 'TM-STARTUP-CANONICAL-CURRENT-OPEN' $platform 'let \(file, path\) = open_verified_local_file\(&launch_path\)\?;' 1 | Out-Null
Require-Count 'TM-STARTUP-READBACK-VERIFIED-OPEN' $platform 'let \(file, resolved_path\) = open_verified_local_file\(&path\)\?;' 1 | Out-Null
$verifiedOpenCount = 2
$noFollowOpenCount = Require-Count 'TM-STARTUP-SAME-HANDLE-NO-FOLLOW' $platform 'crate::windows::open_regular_no_follow\(path\)' 1
Require-Count 'TM-STARTUP-NO-ORDINARY-FILE-OPEN' $platform '\bFile::open\(' 0 | Out-Null
Require-Count 'TM-STARTUP-FINAL-HANDLE-PATH' $platform '\bGetFinalPathNameByHandleW\(' 1 | Out-Null
Require-Count 'TM-STARTUP-FINAL-HANDLE-CANONICAL' $platform 'let resolved_path = resolved_local_path\(&file\)\?;' 1 | Out-Null
Require-Count 'TM-STARTUP-MALFORMED-NUL-SLOT' $platform 'expected_units\s*\.checked_add\(1\)' 1 | Out-Null
Require-Count 'TM-STARTUP-MALFORMED-MORE-DATA' $platform 'read_result == ERROR_MORE_DATA' 1 | Out-Null
$staleBeforeOpen = $platform.IndexOf('if path != current.path {', [System.StringComparison]::Ordinal)
$registeredOpen = $platform.LastIndexOf('let (file, resolved_path) = open_verified_local_file(&path)?;', [System.StringComparison]::Ordinal)
if ($staleBeforeOpen -lt 0 -or $registeredOpen -lt 0 -or $staleBeforeOpen -gt $registeredOpen) {
    throw 'TM-STARTUP-NO-ALTERNATE-PATH-OPEN: stale path must fail before filesystem I/O'
}
Require-Count 'TM-STARTUP-EXPLICIT-STALE-ENABLE' $platform '\(CurrentUserStartupAction::Enable, CurrentUserStartupStatus::StaleRelocation\)' 1 | Out-Null
Require-Count 'TM-STARTUP-EXPLICIT-STALE-REPAIR' $platform '\(CurrentUserStartupAction::RepairStale, CurrentUserStartupStatus::StaleRelocation\)' 1 | Out-Null
Require-Count 'TM-STARTUP-CONFLICT-NONMUTATION' $platform '\(_, CurrentUserStartupStatus::Conflict\)' 1 | Out-Null
Require-Count 'TM-STARTUP-PUBLIC-EXPORT' $platformLib 'CurrentUserStartupSnapshot, CurrentUserStartupStatus' 1 | Out-Null

$forbiddenPlatform = [ordered]@{
    'TM-STARTUP-NO-PROCESS' = '(?m)\bstd::process\b|\bCommand::new\b'
    'TM-STARTUP-NO-SHELL' = '(?i)ShellExecute|cmd\.exe|powershell|pwsh\.exe'
    'TM-STARTUP-NO-ELEVATION' = '(?i)runas|elevat|administrator'
    'TM-STARTUP-NO-POLLING' = '(?m)\bloop\s*\{|thread::spawn|thread::sleep|std::thread'
    'TM-STARTUP-NO-ARBITRARY-PUBLIC-PATH' = '(?m)pub\s+fn\s+\w+\s*\([^)]*(Path|PathBuf|HKEY|PCWSTR|&str|String)'
}
foreach ($entry in $forbiddenPlatform.GetEnumerator()) {
    if ($platform -match $entry.Value) {
        throw "$($entry.Key): forbidden authority detected"
    }
}

$applicationInspectCount = Require-Count 'TM-STARTUP-APP-INSPECT' $application 'CurrentUserStartup::inspect\(\)' 1
$applicationApplyCount = Require-Count 'TM-STARTUP-APP-APPLY' $application 'CurrentUserStartup::apply\(action\)' 1
Require-Count 'TM-STARTUP-APP-TYPED-PORT' $application 'trait ApplicationCurrentUserStartupPort' 1 | Out-Null
Require-Count 'TM-STARTUP-APP-PRESENTER' $application 'current_user_startup_presenter\(\)' 1 | Out-Null
foreach ($intent in @('EnableCurrentUserStartup', 'RepairCurrentUserStartup', 'DisableCurrentUserStartup')) {
    Require-Count "TM-STARTUP-APP-$($intent.ToUpperInvariant())" $application "DesktopIntent::$intent\s*=>" 1 | Out-Null
    Require-Count "TM-STARTUP-DESKTOP-$($intent.ToUpperInvariant())" $desktopState "(?m)^\s{4}$intent,\s*$" 1 | Out-Null
    Require-Count "TM-STARTUP-WIRE-$($intent.ToUpperInvariant())" $desktopUi "DesktopIntent::$intent\)" 1 | Out-Null
}

foreach ($callback in @('enable-current-user-startup', 'repair-current-user-startup', 'disable-current-user-startup')) {
    Require-Count "TM-STARTUP-ROOT-$($callback.ToUpperInvariant())" $mainUi "callback $callback\(\);" 1 | Out-Null
    Require-Count "TM-STARTUP-VIEW-$($callback.ToUpperInvariant())" $settingsUi "callback $callback\(\);" 1 | Out-Null
    Require-Count "TM-STARTUP-MAIN-FORWARD-$($callback.ToUpperInvariant())" $mainUi "$callback\s*=>\s*\{\s*root\.$callback\(\);\s*\}" 1 | Out-Null
}
Require-Count 'TM-STARTUP-VIEW-FORWARD-ENABLE' $settingsUi 'clicked\s*=>\s*\{\s*root\.enable-current-user-startup\(\);\s*\}' 1 | Out-Null
Require-Count 'TM-STARTUP-VIEW-FORWARD-REPAIR' $settingsUi 'clicked\s*=>\s*\{\s*root\.repair-current-user-startup\(\);\s*\}' 1 | Out-Null
Require-Count 'TM-STARTUP-VIEW-FORWARD-DISABLE' $settingsUi 'clicked\s*=>\s*\{\s*root\.disable-current-user-startup\(\);\s*\}' 2 | Out-Null

$accessibleLabels = @(
    'Enable TokenMaster at Windows sign-in',
    'Disable TokenMaster at Windows sign-in',
    'Repair TokenMaster startup registration',
    'Remove old TokenMaster startup registration'
)
foreach ($label in $accessibleLabels) {
    Require-Count 'TM-STARTUP-ACCESSIBILITY' $settingsUi ([regex]::Escape("accessible-label: `"$label`";")) 1 | Out-Null
}
Require-Count 'TM-STARTUP-CONDITIONAL-CONTROLS' $settingsUi 'if root\.current-user-startup-can-' 4 | Out-Null

$portableFiles = @(
    Get-ChildItem -LiteralPath (Join-Path $root 'crates/state/src') -Filter '*.rs' -File -Recurse
    Get-Item -LiteralPath (Join-Path $root 'crates/app/src/state.rs')
    Get-Item -LiteralPath (Join-Path $root 'crates/app/src/command.rs')
) | Sort-Object FullName -Unique
$settingsSources = $portableFiles |
    Sort-Object FullName |
    ForEach-Object { [System.IO.File]::ReadAllText($_.FullName) }
$settingsCombined = $settingsSources -join "`n"
if ($settingsCombined -match '(?i)current.?user.?startup|startup_enabled|start_at_login') {
    throw 'TM-STARTUP-PORTABLE-EXCLUSION: startup state entered reliable or portable settings'
}

[ordered]@{
    result = 'pass'
    scope = 'source-only'
    registry_feature_count = $registryFeatureCount
    hkcu_reference_count = $hkcuCount
    registry_readback_call_count = $readbackCount
    registry_write_call_count = $writeCount
    registry_delete_call_count = $deleteCount
    current_executable_source_count = $currentExecutableCount
    physical_identity_reference_count = $physicalIdentityCount
    run_command_utf16_limit = 260
    run_command_limit_source_count = $commandLimitCount
    local_drive_type_call_count = $driveTypeCount
    alternate_registered_path_open_count = 0
    same_handle_no_follow_open_count = $noFollowOpenCount
    verified_local_open_call_count = $verifiedOpenCount
    application_inspect_count = $applicationInspectCount
    application_apply_count = $applicationApplyCount
    typed_intent_count = 3
    accessible_action_count = $accessibleLabels.Count
    portable_startup_field_count = 0
    portable_codec_scanned_file_count = $portableFiles.Count
    shell_process_polling_authority_count = 0
} | ConvertTo-Json -Compress
