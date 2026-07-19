[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$RepositoryRoot,
    [switch]$SourceOnly
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$rootManifest = Join-Path $root 'Cargo.toml'
$desktopRoot = Join-Path $root 'crates\desktop'
$desktopManifest = Join-Path $desktopRoot 'Cargo.toml'
$sourceRoot = Join-Path $desktopRoot 'src'
$uiRoot = Join-Path $desktopRoot 'ui'

foreach ($required in @($rootManifest, $desktopManifest, $sourceRoot, $uiRoot)) {
    if (-not (Test-Path -LiteralPath $required)) {
        throw "TM-DESKTOP-MISSING-BOUNDARY: $([System.IO.Path]::GetFileName($required))"
    }
}

$manifestText = [System.IO.File]::ReadAllText($desktopManifest)
$devBoundary = $manifestText.IndexOf('[dev-dependencies]', [System.StringComparison]::Ordinal)
$productionManifestText = if ($devBoundary -ge 0) {
    $manifestText.Substring(0, $devBoundary)
} else {
    $manifestText
}

if ($manifestText -match '\btokenmaster-m0\b|[\\/]probe-app\b') {
    throw 'TM-DESKTOP-PROBE-DEPENDENCY: production desktop must not depend on the M0 probe'
}
if ($manifestText -match '\brenderer-femtovg\b') {
    throw 'TM-DESKTOP-FEMTOVG: production desktop must remain software-renderer only'
}
if ($productionManifestText -match '\btokenmaster-(store|provider|runtime|codex|git|platform)\b|\b(rusqlite|libsqlite3-sys|notify)\b') {
    throw 'TM-DESKTOP-DIRECT-AUTHORITY: desktop manifest contains a forbidden direct authority dependency'
}

$rustFiles = @(Get-ChildItem -LiteralPath $sourceRoot -Recurse -File -Filter '*.rs')
$uiFiles = @(Get-ChildItem -LiteralPath $uiRoot -Recurse -File -Filter '*.slint')
$productionFiles = @($rustFiles + $uiFiles)
if ($rustFiles.Count -ne 10 -or $uiFiles.Count -ne 16) {
    throw 'TM-DESKTOP-FILE-COUNT: production desktop boundary must contain ten Rust and sixteen Slint files'
}
$uiText = ($uiFiles | ForEach-Object {
    [System.IO.File]::ReadAllText($_.FullName)
}) -join "`n"
$productionText = ($productionFiles | ForEach-Object {
    [System.IO.File]::ReadAllText($_.FullName)
}) -join "`n"

if ($productionText -match '\b(seed_probe_models|mock|fixture|seeded|seed)\b') {
    throw 'TM-DESKTOP-MOCK-DATA: production desktop contains mock or seeded data'
}
$forbiddenAuthorityPattern = @(
    'https?://',
    '\bstd::(fs|net|process)\b',
    '\b(Command|TcpStream|TcpListener|UdpSocket)\b',
    '\b(rusqlite|notify|reqwest|ureq|webbrowser|headless_chrome)\b',
    '\b(SELECT|INSERT|UPDATE|DELETE\s+FROM|PRAGMA)\b',
    'powershell(?:\.exe)?|cmd(?:\.exe)?|bash(?:\.exe)?|\bsh\s+-c\b',
    'auth\.json|[\\/]\.codex[\\/]auth|\bAuthorization\b|\bBearer\s'
) -join '|'
if ($productionText -cmatch $forbiddenAuthorityPattern) {
    throw 'TM-DESKTOP-FORBIDDEN-AUTHORITY: desktop source contains filesystem/network/process/SQL/browser/credential authority'
}

$controllerPath = Join-Path $sourceRoot 'controller.rs'
$controllerText = [System.IO.File]::ReadAllText($controllerPath)
$bridgePath = Join-Path $sourceRoot 'bridge.rs'
$bridgeText = [System.IO.File]::ReadAllText($bridgePath)
$uiRustPath = Join-Path $sourceRoot 'ui.rs'
$uiRustText = [System.IO.File]::ReadAllText($uiRustPath)
$reliableStateText = [System.IO.File]::ReadAllText((Join-Path $sourceRoot 'reliable_state.rs'))
$workerConstructionCount = [regex]::Matches($controllerText, 'RefreshWorker::spawn\(').Count
if ($workerConstructionCount -ne 1) {
    throw 'TM-DESKTOP-CONTROLLER-WORKER: desktop controller must construct exactly one bounded refresh worker'
}
$snapshotSlotCount = [regex]::Matches(
    $productionText,
    'Arc<Mutex<Option<Arc<ProductSnapshot>>>>'
).Count
if ($snapshotSlotCount -ne 1) {
    throw 'TM-DESKTOP-CONTROLLER-SLOT: desktop and bridge must share exactly one latest product snapshot slot'
}
$bridgeEventScheduleCount = [regex]::Matches($bridgeText, 'slint::invoke_from_event_loop\(').Count
if ($bridgeEventScheduleCount -ne 1) {
    throw 'TM-DESKTOP-BRIDGE-EVENT: desktop bridge must contain exactly one event-loop scheduling site'
}
$reliableEventScheduleCount = [regex]::Matches($uiRustText, 'slint::invoke_from_event_loop\(').Count
if ($reliableEventScheduleCount -ne 1) {
    throw 'TM-DESKTOP-RELIABLE-EVENT: reliable-state delivery must contain exactly one event-loop scheduling site'
}
$eventScheduleCount = [regex]::Matches($productionText, 'slint::invoke_from_event_loop\(').Count
if ($eventScheduleCount -ne 2) {
    throw 'TM-DESKTOP-EVENT-SITES: desktop must contain exactly two bounded event-loop scheduling sites'
}
if ($bridgeText -notmatch 'window:\s*slint::Weak<MainWindow>') {
    throw 'TM-DESKTOP-BRIDGE-WEAK: desktop bridge must retain only a weak Slint window handle'
}
if ($bridgeText -match 'window:\s*MainWindow|\b(slint::Timer|std::thread|thread::spawn|thread::sleep)\b') {
    throw 'TM-DESKTOP-BRIDGE-POLLING: desktop bridge must not retain a strong window, timer, or polling thread'
}
if ($uiRustText -notmatch 'window:\s*slint::Weak<MainWindow>' -or
    $uiRustText -notmatch 'latest:\s*Mutex<Option<DesktopReliableStateProjection>>' -or
    $uiRustText -notmatch 'scheduled:\s*AtomicBool') {
    throw 'TM-DESKTOP-RELIABLE-SLOT: reliable-state delivery must use one latest-only slot, one atomic gate, and a weak window'
}
if ($uiRustText -match '\bVecDeque\b|\b(?:sync_)?channel\b') {
    throw 'TM-DESKTOP-RELIABLE-QUEUE: reliable-state delivery must not retain an unbounded or ordered event queue'
}
if ($reliableStateText -notmatch 'pub struct DesktopRecoveryReceipt' -or
    $reliableStateText -notmatch 'reconstructed_from_authoritative_source' -or
    $reliableStateText -notmatch 'non_reconstructible_domains_lost' -or
    $uiRustText -notmatch 'set_reliable_recovery_kind' -or
    $uiRustText -notmatch 'set_reliable_non_reconstructible_domains_lost' -or
    $uiText -notmatch 'Previous quota, reset-credit, reminder, and Git history is unavailable\.') {
    throw 'TM-DESKTOP-RECOVERY-RECEIPT: durable recovery loss must remain explicit and visible'
}
$uiAdapterText = [System.IO.File]::ReadAllText((Join-Path $sourceRoot 'ui.rs')) + "`n" +
    $uiText
if ($uiAdapterText -match 'QueryService::|RefreshWorker::|DesktopController::|\.usage_analytics\(|\.usage_session_detail\(|\.quota_overview\(|\.benefit_overview\(') {
    throw 'TM-DESKTOP-UI-QUERY: Slint callbacks must not perform controller or query work'
}
if ($controllerText -match 'QuotaCurrentRequest::new\s*\(\s*Vec::new\(\)\s*\)') {
    throw 'TM-DESKTOP-EMPTY-FILTER-DISCOVERY: exact-empty quota filters must not be used for dashboard discovery'
}
if ($uiText -match '(?i)\b(?:text|label|title)\s*:\s*"[^"\r\n]*\b(?:5[ -]?(?:h|hour)|five[ -]?hour|weekly)\b') {
    throw 'TM-DESKTOP-FIXED-QUOTA-ROW: quota rows must be discovered dynamically'
}
if ($uiText -match '(?m)\bdashboard-(?:header-(?:tokens|cost|events)|code-(?:commits|added|removed|net|efficiency))\s*:\s*"(?:\$|\+|-|−)?[0-9]') {
    throw 'TM-DESKTOP-SEEDED-DASHBOARD: dashboard metrics must come from the immutable product snapshot'
}
if ($uiAdapterText -match '(?i)\b(?:account|workspace|window|lot|repo|repository|session|event|source)[_-]?id\b') {
    throw 'TM-DESKTOP-PRIVATE-IDENTITY: private opaque identities must not cross the UI boundary'
}
$modelsText = [System.IO.File]::ReadAllText((Join-Path $uiRoot 'models.slint'))
if ($modelsText -match '(?i)passphrase|password|confirmation') {
    throw 'TM-DESKTOP-SECRET-MODEL: passphrases must never enter a Slint list or global model'
}
if (
    $uiAdapterText -match '(?i)\b(?:slint::Timer|std::thread|thread::spawn|thread::sleep)\b' -or
    $uiText -match '(?i)(?:\bTimer\s*\{|\banimate\s+[A-Za-z_-]+\b|\banimation-[A-Za-z_-]+\b)'
) {
    throw 'TM-DESKTOP-UI-POLLING: UI must remain timer animation and polling free'
}
if ($uiAdapterText -match '(?i)\b(?:WhereMyTokens|WhereMyToken|WhereMyTokens)\b') {
    throw 'TM-DESKTOP-LEGACY-PRODUCT: production UI must contain only TokenMaster product identity'
}

$dashboardPath = Join-Path $sourceRoot 'dashboard.rs'
$dashboardText = [System.IO.File]::ReadAllText($dashboardPath)
$dashboardBounds = [ordered]@{
    DESKTOP_DASHBOARD_SECTION_COUNT = 6
    MAX_DASHBOARD_QUOTA_ROWS = 32
    MAX_DASHBOARD_BENEFIT_SCOPES = 32
    MAX_DASHBOARD_TREND_POINTS = 240
    MAX_DASHBOARD_SESSIONS = 12
    DASHBOARD_ACTIVITY_ROWS = 8
    MAX_DASHBOARD_MODELS = 12
    MAX_DASHBOARD_REPOSITORIES = 32
}
foreach ($bound in $dashboardBounds.GetEnumerator()) {
    $pattern = "pub const $([regex]::Escape($bound.Key)): usize = $($bound.Value);"
    if ($dashboardText -notmatch $pattern) {
        throw "TM-DESKTOP-DASHBOARD-BOUND: $($bound.Key) drifted"
    }
}
foreach ($requiredBoundUse in @(
    '\.take\(MAX_DASHBOARD_QUOTA_ROWS\)',
    '\.take\(MAX_DASHBOARD_BENEFIT_SCOPES\)',
    '\.take\(MAX_DASHBOARD_TREND_POINTS\)',
    '\.take\(MAX_DASHBOARD_SESSIONS\)',
    '\.take\(MAX_DASHBOARD_MODELS\)',
    '\.take\(MAX_DASHBOARD_REPOSITORIES\)'
)) {
    if ($dashboardText -notmatch $requiredBoundUse) {
        throw "TM-DESKTOP-DASHBOARD-BOUND: missing bounded projection $requiredBoundUse"
    }
}
$dashboardProjectionCallCount = [regex]::Matches($uiRustText, 'apply_dashboard_projection\(').Count
if ($dashboardProjectionCallCount -ne 2) {
    throw 'TM-DESKTOP-DASHBOARD-REBUILD: dashboard models must not rebuild during route-only selection'
}
$historyPath = Join-Path $sourceRoot 'history.rs'
$historyText = [System.IO.File]::ReadAllText($historyPath)
if ($historyText -notmatch 'pub const MAX_HISTORY_DAYS: usize = 30;' -or
    $historyText -notmatch '\.take\(MAX_HISTORY_DAYS\)') {
    throw 'TM-DESKTOP-HISTORY-BOUND: history projection must retain at most thirty daily rows'
}
if ($controllerText -notmatch 'pub const HISTORY_DAYS: u16 = 30;' -or
    $controllerText -notmatch 'UsageRange::recent_days\(Self::HISTORY_DAYS\)') {
    throw 'TM-DESKTOP-HISTORY-REQUEST: history query must remain one fixed bounded recent-days request'
}
$historyProjectionCallCount = [regex]::Matches($uiRustText, 'apply_history_projection\(').Count
if ($historyProjectionCallCount -ne 2) {
    throw 'TM-DESKTOP-HISTORY-REBUILD: history models must not rebuild during route-only selection'
}
$historyModelReplacementCount = [regex]::Matches(
    $uiRustText,
    'set_history_day_rows\(model\(rows\)\)'
).Count
if ($historyModelReplacementCount -ne 1) {
    throw 'TM-DESKTOP-HISTORY-MODEL: history must have one bounded model replacement site'
}
$sessionsPath = Join-Path $sourceRoot 'sessions.rs'
$sessionsText = [System.IO.File]::ReadAllText($sessionsPath)
if ($sessionsText -notmatch 'pub const MAX_SESSION_ROWS: usize = 64;' -or
    $sessionsText -notmatch '\.take\(MAX_SESSION_ROWS\)') {
    throw 'TM-DESKTOP-SESSIONS-BOUND: sessions projection must retain at most sixty-four rows'
}
if ($controllerText -notmatch 'pub const MAX_SESSION_ROWS: usize = 64;' -or
    $controllerText -notmatch 'PageSize::new\(Self::MAX_SESSION_ROWS\)') {
    throw 'TM-DESKTOP-SESSIONS-REQUEST: sessions query must remain one bounded first page'
}
$sessionsProjectionCallCount = [regex]::Matches($uiRustText, 'apply_sessions_projection\(').Count
if ($sessionsProjectionCallCount -ne 2) {
    throw 'TM-DESKTOP-SESSIONS-REBUILD: sessions models must not rebuild during route-only selection'
}
$sessionsModelReplacementCount = [regex]::Matches(
    $uiRustText,
    'set_session_list_rows\(model\(rows\)\)'
).Count
if ($sessionsModelReplacementCount -ne 1) {
    throw 'TM-DESKTOP-SESSIONS-MODEL: sessions must have one bounded model replacement site'
}
$sessionDetailBounds = [ordered]@{
    MAX_SESSION_DETAIL_MODEL_ROWS = 32
    MAX_SESSION_DETAIL_PROJECT_ROWS = 32
}
foreach ($bound in $sessionDetailBounds.GetEnumerator()) {
    $pattern = "pub const $([regex]::Escape($bound.Key)): usize = $($bound.Value);"
    if ($sessionsText -notmatch $pattern) {
        throw "TM-DESKTOP-SESSION-DETAIL-BOUND: $($bound.Key) drifted"
    }
}
if ($sessionsText -notmatch 'Vec::with_capacity\(MAX_SESSION_DETAIL_MODEL_ROWS \+ MAX_SESSION_DETAIL_PROJECT_ROWS\)' -or
    $sessionsText -notmatch 'model_count >= MAX_SESSION_DETAIL_MODEL_ROWS' -or
    $sessionsText -notmatch 'project_count >= MAX_SESSION_DETAIL_PROJECT_ROWS') {
    throw 'TM-DESKTOP-SESSION-DETAIL-BOUND: exact session detail must retain at most 32 model and 32 project rows'
}
$sessionDetailQueuePattern = '(?im)^(?=[^\r\n]*(?:session|detail))[^\r\n]*(?:\b(?:VecDeque|HashMap|BTreeMap|LinkedList)\b|\bVec\s*<|\b(?:sync_)?channel\s*(?:::)?\s*(?:<|\())'
if ($controllerText -notmatch 'pending_selection:\s*Option<PendingDesktopSessionDetail>' -or
    $controllerText -notmatch 'latest_selection_generation:\s*Option<ProductSessionDetailSelectionGeneration>' -or
    $controllerText -match $sessionDetailQueuePattern) {
    throw 'TM-DESKTOP-SESSION-DETAIL-SLOT: exact detail work must use one latest-only typed slot'
}
$presentationText = [System.IO.File]::ReadAllText((Join-Path $sourceRoot 'presentation.rs'))
$sessionUiBoundaryText = $sessionsText + "`n" + $presentationText + "`n" + $uiRustText + "`n" + $uiText
if ($sessionUiBoundaryText -match '\bUsageSessionKey\b') {
    throw 'TM-DESKTOP-SESSION-DETAIL-IDENTITY: opaque session keys must remain inside the controller worker'
}
if ($controllerText -notmatch 'source\s*\.usage_session_detail\(key\)' -or
    $controllerText -notmatch 'DesktopSessionDetailIntent' -or
    $uiText -notmatch 'callback select-session\(int\)' -or
    $uiRustText -notmatch 'window\.on_select_session\(' -or
    $uiText -notmatch 'row-focus := FocusScope' -or
    $uiText -notmatch 'focus-on-tab-navigation:\s*true' -or
    $uiText -notmatch 'row-focus\.focus\(\)' -or
    $uiText -notmatch 'row-touch\.has-hover' -or
    $uiText -notmatch 'accessible-action-default') {
    throw 'TM-DESKTOP-SESSION-DETAIL-ROUTING: typed selection must route through the controller worker'
}
$sessionDetailModelReplacementCount = [regex]::Matches(
    $uiRustText,
    'set_session_detail_breakdown_rows\(model\(rows\)\)'
).Count
if ($sessionDetailModelReplacementCount -ne 1) {
    throw 'TM-DESKTOP-SESSION-DETAIL-MODEL: exact detail must have one bounded model replacement site'
}
$reliableStatePath = Join-Path $sourceRoot 'reliable_state.rs'
$reliableStateText = [System.IO.File]::ReadAllText($reliableStatePath)
if ($reliableStateText -notmatch 'pub const MAX_DESKTOP_RESTORE_POINTS: usize = 15;' -or
    $reliableStateText -notmatch '\.take\(MAX_DESKTOP_RESTORE_POINTS\)') {
    throw 'TM-DESKTOP-RESTORE-BOUND: reliable-state projection must retain at most fifteen points'
}
$restoreModelReplacementCount = [regex]::Matches($uiRustText, 'set_restore_point_rows\(model\(rows\)\)').Count
if ($restoreModelReplacementCount -ne 1) {
    throw 'TM-DESKTOP-RESTORE-MODEL: restore-point model must have one bounded replacement site'
}
if ($uiRustText -notmatch 'reviewed_restore_selection = Rc::new\(RefCell::new\(None\)\)' -or
    $uiRustText -notmatch 'reviewed_selection\.replace\(Some\(selection\)\)' -or
    $uiRustText -notmatch 'let selection = \*reviewed_selection\.borrow\(\)') {
    throw 'TM-DESKTOP-RESTORE-IDENTITY: confirmation must retain the exact reviewed generation and ordinal'
}
if ($reliableStateText -notmatch 'successful_count: Option<u64>' -or
    $reliableStateText -notmatch 'failure_count: Option<u64>' -or
    $reliableStateText -notmatch 'published_bytes: Option<u64>' -or
    [regex]::Matches($uiRustText, 'map_or_else\(\|\| "Unavailable"\.to_owned\(\)').Count -lt 3) {
    throw 'TM-DESKTOP-UNKNOWN-METRICS: unavailable metrics must remain typed unknowns in the UI'
}
foreach ($requiredIntent in @(
    'callback export-config\(\)',
    'callback import-config\(\)',
    'callback confirm-config-import\(\)',
    'callback cancel-config-import\(\)',
    'callback backup-normal\(\)',
    'callback backup-compact\(\)',
    'callback backup-encrypted\(string, string\)',
    'callback verify-backups\(\)',
    'callback preview-restore\(int\)',
    'callback confirm-restore\(int, bool\)',
    'callback rebuild-data\(\)',
    'callback retry-operation\(\)',
    'callback cancel-operation\(\)',
    'callback update-backup-policy\(bool, int, int, int\)'
)) {
    if ($uiText -notmatch $requiredIntent) {
        throw "TM-DESKTOP-RELIABLE-INTENT: missing typed intent $requiredIntent"
    }
}
if ($uiText -notmatch 'passphrase\.text\s*=\s*""' -or
    $uiText -notmatch 'confirmation\.text\s*=\s*""') {
    throw 'TM-DESKTOP-SECRET-CLEAR: transient passphrase fields must clear after admission'
}
foreach ($requiredPolicyBound in @(
    'minimum:\s*300;\s*maximum:\s*3600',
    'minimum:\s*21600;\s*maximum:\s*604800',
    'minimum:\s*256;\s*maximum:\s*65536'
)) {
    if ($uiText -notmatch $requiredPolicyBound) {
        throw "TM-DESKTOP-POLICY-BOUND: backup policy control drifted: $requiredPolicyBound"
    }
}
$dashboardModelReplacementCount = [regex]::Matches(
    $uiRustText,
    'set_dashboard_(?:section_rows|quota_rows|benefit_rows|trend_points|session_rows|activity_rows|model_rows)\(model\('
).Count
if ($dashboardModelReplacementCount -ne 7) {
    throw 'TM-DESKTOP-DASHBOARD-MODEL: dashboard must replace each of seven bounded list models exactly once'
}

$presentationPath = Join-Path $sourceRoot 'presentation.rs'
$presentationText = [System.IO.File]::ReadAllText($presentationPath)
$stableStart = $presentationText.IndexOf('pub const fn stable_key', [System.StringComparison]::Ordinal)
$stableEnd = $presentationText.IndexOf('pub const fn label_key', [System.StringComparison]::Ordinal)
if ($stableStart -lt 0 -or $stableEnd -le $stableStart) {
    throw 'TM-DESKTOP-ROUTE-COUNT: stable route-key boundary is absent'
}
$stableSection = $presentationText.Substring($stableStart, $stableEnd - $stableStart)
$expectedRouteKeys = @(
    'dashboard', 'history', 'sessions', 'models', 'projects', 'activity',
    'data_health', 'notifications', 'settings', 'help_about', 'compact_widget'
)
$routeMatches = [regex]::Matches($stableSection, 'Self::[A-Za-z]+\s*=>\s*"([a-z_]+)"')
$actualRouteKeys = @($routeMatches | ForEach-Object { $_.Groups[1].Value })
if (
    $actualRouteKeys.Count -ne 11 -or
    @($expectedRouteKeys | Where-Object { $_ -notin $actualRouteKeys }).Count -ne 0 -or
    @($actualRouteKeys | Sort-Object -Unique).Count -ne 11
) {
    throw 'TM-DESKTOP-ROUTE-COUNT: desktop route keys drifted from the fixed 11-route contract'
}

foreach ($requiredPattern in @(
    'pub const DESKTOP_ROUTE_COUNT: usize = ProductRoute::ALL\.len\(\)',
    'values: \[Option<&''static str>; MAX_ROUTE_REASONS\]',
    'const MAX_ROUTE_REASONS: usize = 11',
    'DesktopApplyOutcome::IgnoredNotNewer',
    'std::array::from_fn',
    'ModelRc::new\(VecModel::from\(rows\)\)',
    'ProductReducer::new\(\)',
    'reducer\.snapshot\(\)',
    'winit-software'
)) {
    if ($productionText -notmatch $requiredPattern) {
        throw "TM-DESKTOP-MISSING-CONTRACT: $requiredPattern"
    }
}
if ($productionText -match '\b(QuotaRow|SessionRow|ChartPoint|quota-targets|chart-points)\b') {
    throw 'TM-DESKTOP-MOCK-DATA: production shell contains probe data models'
}

if ($SourceOnly) {
    [ordered]@{
        result = 'pass'
        scope = 'source-only'
        fixed_route_count = 11
        rust_source_file_count = $rustFiles.Count
        slint_source_file_count = $uiFiles.Count
        controller_worker_count = $workerConstructionCount
        retained_snapshot_slot_count = $snapshotSlotCount
        event_loop_schedule_site_count = $eventScheduleCount
        bridge_event_loop_schedule_site_count = $bridgeEventScheduleCount
        reliable_event_loop_schedule_site_count = $reliableEventScheduleCount
        bridge_polling_surface_count = 0
        dashboard_section_count = $dashboardBounds.DESKTOP_DASHBOARD_SECTION_COUNT
        dashboard_model_replacement_count = $dashboardModelReplacementCount
        dashboard_projection_application_count = $dashboardProjectionCallCount - 1
        dashboard_polling_surface_count = 0
        history_day_maximum = 30
        history_model_replacement_count = $historyModelReplacementCount
        history_projection_application_count = $historyProjectionCallCount - 1
        history_polling_surface_count = 0
        session_row_maximum = 64
        session_detail_model_row_maximum = 32
        session_detail_project_row_maximum = 32
        sessions_model_replacement_count = $sessionsModelReplacementCount
        session_detail_model_replacement_count = $sessionDetailModelReplacementCount
        sessions_projection_application_count = $sessionsProjectionCallCount - 1
        sessions_polling_surface_count = 0
        restore_point_maximum = 15
        restore_model_replacement_count = $restoreModelReplacementCount
        secret_model_count = 0
        private_ui_identity_count = 0
    } | ConvertTo-Json -Compress
    return
}

$metadataJson = & cargo +1.97.0 metadata --locked --format-version 1 --manifest-path $rootManifest
if ($LASTEXITCODE -ne 0) {
    throw 'TM-DESKTOP-METADATA: cargo metadata failed'
}
$metadata = $metadataJson | ConvertFrom-Json -Depth 100
$desktopPackages = @($metadata.packages | Where-Object { $_.name -eq 'tokenmaster-desktop' })
if ($desktopPackages.Count -ne 1) {
    throw 'TM-DESKTOP-PACKAGE: tokenmaster-desktop must resolve exactly once'
}
$directProductionDependencies = @(
    $desktopPackages[0].dependencies |
        Where-Object { $null -eq $_.kind } |
        ForEach-Object { $_.name } |
        Sort-Object -Unique
)
$expectedDependencies = @(
    'anyhow', 'chrono', 'slint', 'tokenmaster-domain', 'tokenmaster-engine',
    'tokenmaster-product', 'tokenmaster-query'
)
if (
    $directProductionDependencies.Count -ne $expectedDependencies.Count -or
    @($expectedDependencies | Where-Object { $_ -notin $directProductionDependencies }).Count -ne 0
) {
    throw "TM-DESKTOP-DIRECT-AUTHORITY: direct dependency set drifted: $($directProductionDependencies -join ', ')"
}

$featureTree = (& cargo +1.97.0 tree -p tokenmaster-desktop -e features --manifest-path $rootManifest) -join "`n"
if ($LASTEXITCODE -ne 0) {
    throw 'TM-DESKTOP-TREE: cargo feature tree failed'
}
if ($featureTree -notmatch 'renderer-software') {
    throw 'TM-DESKTOP-SOFTWARE-RENDERER: software renderer is absent'
}
if ($featureTree -match 'renderer-femtovg') {
    throw 'TM-DESKTOP-FEMTOVG: package feature tree contains FemtoVG'
}
if ($featureTree -match 'tokenmaster-m0') {
    throw 'TM-DESKTOP-PROBE-DEPENDENCY: package tree contains the M0 probe'
}

& cargo +1.97.0 build --release --locked --manifest-path $rootManifest -p tokenmaster-desktop
if ($LASTEXITCODE -ne 0) {
    throw 'TM-DESKTOP-BUILD: release desktop build failed'
}

[ordered]@{
    result = 'pass'
    package = 'tokenmaster-desktop'
    binary = $null
    direct_production_dependencies = $directProductionDependencies
    rust_source_file_count = $rustFiles.Count
    slint_source_file_count = $uiFiles.Count
    fixed_route_count = 11
    maximum_route_reason_count = 11
    retained_route_model_count = 1
    controller_worker_count = $workerConstructionCount
    retained_snapshot_slot_count = $snapshotSlotCount
    event_loop_schedule_site_count = $eventScheduleCount
    bridge_event_loop_schedule_site_count = $bridgeEventScheduleCount
    reliable_event_loop_schedule_site_count = $reliableEventScheduleCount
    bridge_polling_surface_count = 0
    dashboard_section_count = $dashboardBounds.DESKTOP_DASHBOARD_SECTION_COUNT
    dashboard_model_replacement_count = $dashboardModelReplacementCount
    dashboard_projection_application_count = $dashboardProjectionCallCount - 1
    dashboard_polling_surface_count = 0
    history_day_maximum = 30
    history_model_replacement_count = $historyModelReplacementCount
    history_projection_application_count = $historyProjectionCallCount - 1
    history_polling_surface_count = 0
    session_row_maximum = 64
    session_detail_model_row_maximum = 32
    session_detail_project_row_maximum = 32
    sessions_model_replacement_count = $sessionsModelReplacementCount
    session_detail_model_replacement_count = $sessionDetailModelReplacementCount
    sessions_projection_application_count = $sessionsProjectionCallCount - 1
    sessions_polling_surface_count = 0
    restore_point_maximum = 15
    restore_model_replacement_count = $restoreModelReplacementCount
    secret_model_count = 0
    private_ui_identity_count = 0
    mock_data_model_count = 0
    direct_authority_dependency_count = 0
    forbidden_source_authority_count = 0
    femtovg_feature_count = 0
    probe_dependency_count = 0
    release_artifact_count = 0
} | ConvertTo-Json -Compress
