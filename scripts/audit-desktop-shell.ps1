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
if ($rustFiles.Count -ne 15 -or $uiFiles.Count -ne 22) {
    throw 'TM-DESKTOP-FILE-COUNT: production desktop boundary must contain fifteen Rust and twenty-two Slint files'
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

$inAppNotificationPath = Join-Path $sourceRoot 'in_app_notification.rs'
$inAppNotificationText = [System.IO.File]::ReadAllText($inAppNotificationPath)
$inAppPanelText = [System.IO.File]::ReadAllText(
    (Join-Path $uiRoot 'components\in-app-notification-panel.slint')
)
$mainUiTextForInApp = [System.IO.File]::ReadAllText((Join-Path $uiRoot 'main.slint'))
if ($inAppNotificationText -notmatch 'pub const MAX_DESKTOP_IN_APP_NOTIFICATIONS: usize = 256;' -or
    $inAppNotificationText -notmatch 'rows\.len\(\) > MAX_DESKTOP_IN_APP_NOTIFICATIONS' -or
    $inAppNotificationText -notmatch 'if rows\.is_empty\(\)') {
    throw 'TM-DESKTOP-IN-APP-BOUND: presentation must retain exactly one to 256 rows'
}
$inAppModelCount = [regex]::Matches(
    $mainUiTextForInApp,
    'property\s*<\[InAppNotificationRow\]>\s+in-app-notification-[A-Za-z0-9_-]+'
).Count
if ($inAppModelCount -ne 1 -or
    [regex]::Matches($uiRustText, 'set_in_app_notification_rows\(model\(rows\)\)').Count -ne 1) {
    throw 'TM-DESKTOP-IN-APP-MODEL: presentation must own one transient notification model'
}
$applyFunction = [regex]::Match(
    $uiRustText,
    '(?s)pub\(crate\) fn apply_in_app_notification_batch\(.*?\r?\n\}\r?\n\r?\nfn notification_coverage_label'
).Value
$rowsIndex = $applyFunction.IndexOf(
    'window.set_in_app_notification_rows(model(rows));',
    [System.StringComparison]::Ordinal
)
$countIndex = $applyFunction.IndexOf(
    'window.set_in_app_notification_count_label(count_label.into());',
    [System.StringComparison]::Ordinal
)
$visibleIndex = $applyFunction.IndexOf(
    'window.set_in_app_notification_visible(true);',
    [System.StringComparison]::Ordinal
)
$verifiedIndex = $applyFunction.IndexOf(
    'window.get_in_app_notification_visible()',
    [System.StringComparison]::Ordinal
)
if ([string]::IsNullOrWhiteSpace($applyFunction) -or $rowsIndex -lt 0 -or
    $countIndex -le $rowsIndex -or $visibleIndex -le $countIndex -or
    $verifiedIndex -le $visibleIndex) {
    throw 'TM-DESKTOP-IN-APP-APPLY: model count and visibility must be applied and verified in order'
}
$successfulApplyCount = [regex]::Matches(
    $inAppNotificationText,
    '(?s)if apply_in_app_notification_batch\(&window, batch\) \{\s*NotificationDeliveryOutcome::Presented'
).Count
$runNotificationFunction = [regex]::Match(
    $inAppNotificationText,
    '(?s)fn run\(.*?\r?\n    \}\r?\n\r?\n    fn record_schedule_error'
).Value
$readyBeforeReceiptCount = [regex]::Matches(
    $runNotificationFunction,
    '(?s)let presented = match self\.delivery\.deliver\(&batch\).*?self\.scheduled\.store\(false, Ordering::Release\);\s*if presented \{\s*receipt\.presented\(\);\s*\} else \{\s*receipt\.failed\(\);'
).Count
$failedDeliveryCount = [regex]::Matches(
    $runNotificationFunction,
    '(?s)NotificationDeliveryOutcome::(?:Stale|WindowClosed|StateUnavailable) => \{.*?false\s*\}'
).Count
if ($successfulApplyCount -ne 1 -or $readyBeforeReceiptCount -ne 1 -or
    $failedDeliveryCount -ne 3 -or
    [regex]::Matches($inAppNotificationText, 'receipt\.presented\(\);').Count -ne 1 -or
    [regex]::Matches($inAppNotificationText, 'receipt\.failed\(\);').Count -ne 2) {
    throw 'TM-DESKTOP-IN-APP-RECEIPT: visible apply and bridge readiness must precede Presented while every callback failure fails'
}
if ($applyFunction -notmatch '\{benefit_label\}\. \{kind_label\}, quantity \{quantity_label\}') {
    throw 'TM-DESKTOP-IN-APP-ACCESSIBILITY: accessible rows must include the visible benefit and kind labels'
}
$inAppEpochGuardCount = 0
if ($inAppNotificationText -match 'self\.epochs\.active\.load\(Ordering::Acquire\) != self\.epoch' -and
    $inAppNotificationText -match 'let epoch = epochs\.activate\(\)\?;' -and
    $inAppNotificationText -match 'self\.epochs\.deactivate\(self\.epoch\);') {
    $inAppEpochGuardCount = 1
}
if ($inAppEpochGuardCount -ne 1) {
    throw 'TM-DESKTOP-IN-APP-EPOCH: presentation must use one checked independently invalidated epoch'
}
$inAppPublicValue = [regex]::Match(
    $inAppNotificationText,
    '(?s)pub struct DesktopInAppNotification\s*\{.*?\r?\n\}'
).Value
if ($inAppPublicValue -match '(?i)\b(?:delivery|provider|account|workspace|scope|lot|window|target|receipt|activation)[_-]?id\b|\b(?:absolute_)?path\b') {
    throw 'TM-DESKTOP-IN-APP-IDENTITY: presentation value must not expose private identity or paths'
}
if ($inAppNotificationText -match '\b(?:VecDeque|sync_channel|std::thread|thread::spawn|thread::sleep|slint::Timer)\b' -or
    $inAppPanelText -match '(?i)(?:\bTimer\s*\{|\banimate\s+[A-Za-z_-]+\b|animation-[A-Za-z_-]+|auto[-_]?hide)') {
    throw 'TM-DESKTOP-IN-APP-OWNER: presentation must not add a queue timer worker polling or auto-hide owner'
}
$fixedUpstreamAttribution = 'WhereMyTokens and ccusage are pinned external MIT references, not runtime dependencies.'
$legacyProductBoundary = $uiAdapterText.Replace($fixedUpstreamAttribution, '')
if ($legacyProductBoundary -match '(?i)\b(?:WhereMyTokens|WhereMyToken|WhereMyTokens)\b') {
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
$modelsProjectionPath = Join-Path $sourceRoot 'models.rs'
$modelsProjectionText = [System.IO.File]::ReadAllText($modelsProjectionPath)
if ($modelsProjectionText -notmatch 'pub const MAX_MODEL_ROWS: usize = 64;' -or
    $modelsProjectionText -notmatch '\.take\(MAX_MODEL_ROWS\)' -or
    $modelsProjectionText -notmatch 'breakdown\.truncated\(\) \|\| breakdown\.items\(\)\.len\(\) > MAX_MODEL_ROWS') {
    throw 'TM-DESKTOP-MODELS-BOUND: Models projection must preserve backend truncation and retain at most sixty-four rows'
}
$analyticsQueryCallCount = [regex]::Matches($controllerText, 'source\.usage_analytics\(').Count
$recentModelsRequestPattern = '(?s)let history = UsageAnalyticsRequest::new\(\s*UsageRange::recent_days\(Self::HISTORY_DAYS\).*?UsageSeriesSelection::Daily,\s*Vec::new\(\),\s*vec!\[\s*UsageBreakdownKind::Model,\s*UsageBreakdownKind::Project,?\s*\],\s*\)'
if ($analyticsQueryCallCount -ne 2 -or $controllerText -notmatch $recentModelsRequestPattern) {
    throw 'TM-DESKTOP-MODELS-REQUEST: Models and Projects must share the one fixed recent analytics request without a third query'
}
$modelsProjectionCallCount = [regex]::Matches($uiRustText, 'apply_models_projection\(').Count
if ($modelsProjectionCallCount -ne 2) {
    throw 'TM-DESKTOP-MODELS-REBUILD: Models rows must not rebuild during route-only selection'
}
$modelsModelReplacementCount = [regex]::Matches(
    $uiRustText,
    'set_model_usage_rows\(model\(rows\)\)'
).Count
if ($modelsModelReplacementCount -ne 1) {
    throw 'TM-DESKTOP-MODELS-MODEL: Models must have one bounded model replacement site'
}
$mainUiText = [System.IO.File]::ReadAllText((Join-Path $uiRoot 'main.slint'))
$modelsViewText = [System.IO.File]::ReadAllText((Join-Path $uiRoot 'views\models-view.slint'))
$requiredModelsViewPatterns = @(
    'if root\.models-visible: ModelsView',
    '!root\.models-visible',
    'out property <bool> narrow:',
    'if root\.narrow:',
    'if !root\.narrow:',
    'model\.input-label',
    'model\.cached-label',
    'model\.output-label',
    'model\.reasoning-label',
    'model\.total-label',
    'model\.cost-label',
    'model\.cost-evidence-label',
    'model\.event-label',
    'root\.total-availability',
    'root\.cost-availability',
    'Text \{ text: "Relative";',
    'accessible-label:'
)
foreach ($requiredPattern in $requiredModelsViewPatterns) {
    $viewBoundary = $mainUiText + "`n" + $modelsViewText
    if ($viewBoundary -notmatch $requiredPattern) {
        throw "TM-DESKTOP-MODELS-VIEW: missing responsive Models contract $requiredPattern"
    }
}
$projectsProjectionPath = Join-Path $sourceRoot 'projects.rs'
$projectsProjectionText = [System.IO.File]::ReadAllText($projectsProjectionPath)
if ($projectsProjectionText -notmatch 'pub const MAX_PROJECT_ROWS: usize = 32;' -or
    $projectsProjectionText -notmatch '\.take\(MAX_PROJECT_ROWS\)' -or
    $projectsProjectionText -notmatch 'breakdown\.truncated\(\) \|\| breakdown\.items\(\)\.len\(\) > MAX_PROJECT_ROWS') {
    throw 'TM-DESKTOP-PROJECTS-BOUND: Projects projection must preserve backend truncation and retain at most thirty-two rows'
}
$gitQueryCallCount = [regex]::Matches($controllerText, 'source\.git_output\(').Count
$todayGitRequestPattern = '(?s)let git = GitOutputRequest::new\(\s*UsageRange::today\(\),\s*WeekStart::Monday,\s*Vec::new\(\),\s*Self::MAX_REPOSITORIES,\s*\)'
if ($gitQueryCallCount -ne 1 -or $controllerText -notmatch $todayGitRequestPattern) {
    throw 'TM-DESKTOP-PROJECTS-REQUEST: Projects must reuse one bounded UTC-today Git request'
}
if ($projectsProjectionText -notmatch 'alias\.as_str\(\) == project' -or
    $projectsProjectionText -match '(?i)contains\(project\)|starts_with\(project\)|ends_with\(project\)|to_lowercase\(\)') {
    throw 'TM-DESKTOP-PROJECTS-JOIN: Projects must join Git only by an exact safe alias'
}
if ($projectsProjectionText -notmatch 'self\.cost = Some\(cost\)' -or
    $projectsProjectionText -match 'self\.cost\s*=\s*self\.cost.*checked_add|efficiency_cost\.checked_add|efficiency_usage\.checked_add') {
    throw 'TM-DESKTOP-PROJECTS-EFFICIENCY: same-alias repositories must count one project cost exactly once'
}
$projectsProjectionCallCount = [regex]::Matches($uiRustText, 'apply_projects_projection\(').Count
if ($projectsProjectionCallCount -ne 2) {
    throw 'TM-DESKTOP-PROJECTS-REBUILD: Projects rows must not rebuild during route-only selection'
}
$projectsModelReplacementCount = [regex]::Matches(
    $uiRustText,
    'set_project_usage_rows\(model\(rows\)\)'
).Count
if ($projectsModelReplacementCount -ne 1) {
    throw 'TM-DESKTOP-PROJECTS-MODEL: Projects must have one bounded model replacement site'
}
$projectsViewText = [System.IO.File]::ReadAllText((Join-Path $uiRoot 'views\projects-view.slint'))
$projectsViewBoundary = $mainUiText + "`n" + $projectsViewText + "`n" + $uiRustText
foreach ($requiredPattern in @(
    'if root\.projects-visible: ProjectsView',
    '!root\.projects-visible',
    'out property <bool> narrow:',
    'if root\.narrow:',
    'if !root\.narrow:',
    'Recent usage',
    'Today code',
    'usage-range-label',
    'code-range-label',
    'project\.input-label',
    'project\.cached-label',
    'project\.output-label',
    'project\.reasoning-label',
    'project\.total-label',
    'project\.cost-label',
    'project\.cost-evidence-label',
    'project\.commits-label',
    'project\.added-label',
    'project\.removed-label',
    'project\.net-label',
    'project\.efficiency-label',
    'project\.code-status-label',
    'project\.efficiency-reason-label',
    'project\.code-evidence-label',
    '"repository_not_linked" => "Not linked"',
    'Cost / 100 added product-code lines',
    'added product-code lines',
    'Text \{ text: "Relative";',
    'accessible-label:.*project\.code-status-label.*project\.efficiency-reason-label'
)) {
    if ($projectsViewBoundary -cnotmatch $requiredPattern) {
        throw "TM-DESKTOP-PROJECTS-VIEW: missing responsive Projects contract $requiredPattern"
    }
}
$projectPublicText = @(
    [regex]::Match($projectsProjectionText, '(?s)pub struct DesktopProjectUsageRow\s*\{.*?\r?\n\}').Value
    [regex]::Match($projectsProjectionText, '(?s)pub struct DesktopProjectsProjection\s*\{.*?\r?\n\}').Value
) -join "`n"
if ($projectPublicText -match '(?i)\b(?:repository|association|dataset|provider|profile|account|session|source|event)[_-]?id\b|\b(?:absolute_)?path\b|\bkey\b|\bcursor\b') {
    throw 'TM-DESKTOP-PROJECTS-IDENTITY: private identity or path crossed the Projects projection boundary'
}
$activityProjectionPath = Join-Path $sourceRoot 'activity.rs'
$activityProjectionText = [System.IO.File]::ReadAllText($activityProjectionPath)
if ($activityProjectionText -notmatch 'pub const MAX_ACTIVITY_ROWS: usize = 12;' -or
    $activityProjectionText -notmatch '\.take\(MAX_ACTIVITY_ROWS\)' -or
    $activityProjectionText -notmatch 'page\.has_more\(\) \|\| truncated') {
    throw 'TM-DESKTOP-ACTIVITY-BOUND: Activity projection must preserve page incompleteness and retain at most twelve rows'
}
$activityQueryCallCount = [regex]::Matches($controllerText, 'source\.latest_activity\(').Count
if ($activityQueryCallCount -ne 1 -or
    $controllerText -notmatch 'pub const MAX_DASHBOARD_ROWS: usize = 12;' -or
    $controllerText -notmatch 'activity: LatestActivityRequest::first\(overview_page_size\)') {
    throw 'TM-DESKTOP-ACTIVITY-REQUEST: Activity must reuse one bounded first-page request on the existing worker'
}
$activityProjectionCallCount = [regex]::Matches($uiRustText, 'apply_activity_route_projection\(').Count
if ($activityProjectionCallCount -ne 2) {
    throw 'TM-DESKTOP-ACTIVITY-REBUILD: Activity rows must not rebuild during route-only selection'
}
$activityModelReplacementCount = [regex]::Matches(
    $uiRustText,
    'set_recent_activity_rows\(model\(rows\)\)'
).Count
if ($activityModelReplacementCount -ne 1) {
    throw 'TM-DESKTOP-ACTIVITY-MODEL: Activity must have one bounded model replacement site'
}
$activityViewText = [System.IO.File]::ReadAllText((Join-Path $uiRoot 'views\activity-view.slint'))
$activityViewBoundary = $mainUiText + "`n" + $activityViewText + "`n" + $uiRustText
foreach ($requiredPattern in @(
    'if root\.activity-visible: ActivityView',
    '!root\.activity-visible',
    'out property <bool> narrow:',
    'if root\.narrow:',
    'if !root\.narrow:',
    'Recent activity',
    'UTC timestamps',
    'More activity available',
    'set_activity_page_available\(activity\.has_more\(\)\.is_some\(\)\)',
    'No activity events in the available page',
    'format_timestamp_utc\(row\.timestamp_seconds\(\), row\.timestamp_nanos\(\)\)',
    'item\.input-label',
    'item\.cached-label',
    'item\.output-label',
    'item\.reasoning-label',
    'item\.total-label',
    'accessible-label:.*item\.input-label.*item\.cached-label.*item\.output-label.*item\.reasoning-label.*item\.total-label'
)) {
    if ($activityViewBoundary -cnotmatch $requiredPattern) {
        throw "TM-DESKTOP-ACTIVITY-VIEW: missing responsive Activity contract $requiredPattern"
    }
}
$activityPublicText = @(
    [regex]::Match($activityProjectionText, '(?s)pub struct DesktopRecentActivityRow\s*\{.*?\r?\n\}').Value
    [regex]::Match($activityProjectionText, '(?s)pub struct DesktopActivityProjection\s*\{.*?\r?\n\}').Value
) -join "`n"
if ($activityPublicText -match '(?i)\b(?:scope|provider|profile|account|session|source|event|cursor|fingerprint|dataset|project|path|key|id)(?:[_-]?id)?\b\s*:') {
    throw 'TM-DESKTOP-ACTIVITY-IDENTITY: private identity or provenance crossed the Activity projection boundary'
}
if ($activityProjectionText -match '\.(?:scope|provider|profile|account|session|source|event_id|cursor|fingerprint|dataset|project|path|key|id)\(\)') {
    throw 'TM-DESKTOP-ACTIVITY-IDENTITY: Activity projection must not read private identity or provenance fields'
}
if ($activityViewBoundary -match '(?i)\b(?:rhythm|heatmap|day-of-week|hourly)\b') {
    throw 'TM-DESKTOP-ACTIVITY-RHYTHM: Recent activity must not claim an unimplemented rhythm or heatmap aggregate'
}
$notificationsProjectionPath = Join-Path $sourceRoot 'notifications.rs'
$notificationsProjectionText = [System.IO.File]::ReadAllText($notificationsProjectionPath)
$notificationBounds = [ordered]@{
    MAX_NOTIFICATION_SCOPES = 32
    MAX_NOTIFICATION_LOTS = 256
    MAX_NOTIFICATION_LEADS = 8
}
foreach ($bound in $notificationBounds.GetEnumerator()) {
    $pattern = "pub const $([regex]::Escape($bound.Key)): usize = $($bound.Value);"
    if ($notificationsProjectionText -notmatch $pattern) {
        throw "TM-DESKTOP-NOTIFICATIONS-BOUND: $($bound.Key) drifted"
    }
}
foreach ($requiredBoundUse in @(
    '\.take\(MAX_NOTIFICATION_SCOPES\)',
    '\.take\(MAX_NOTIFICATION_LEADS\)',
    'lots\.len\(\) == MAX_NOTIFICATION_LOTS'
)) {
    if ($notificationsProjectionText -notmatch $requiredBoundUse) {
        throw "TM-DESKTOP-NOTIFICATIONS-BOUND: missing bounded projection $requiredBoundUse"
    }
}
$benefitQueryCallCount = [regex]::Matches($controllerText, 'source\.benefit_overview\(').Count
if ($benefitQueryCallCount -ne 1 -or
    $controllerText -notmatch 'source\.benefit_overview\(BenefitOverviewRequest::new\(\)\)') {
    throw 'TM-DESKTOP-NOTIFICATIONS-REQUEST: Notifications must reuse one bounded all-current benefit overview'
}
$notificationsProjectionCallCount = [regex]::Matches(
    $uiRustText,
    'apply_notifications_projection\('
).Count
if ($notificationsProjectionCallCount -ne 2) {
    throw 'TM-DESKTOP-NOTIFICATIONS-REBUILD: Notifications models must not rebuild during route-only selection'
}
$notificationScopeModelReplacementCount = [regex]::Matches(
    $uiRustText,
    'set_reminder_scope_rows\(model\(scope_rows\)\)'
).Count
$notificationLotModelReplacementCount = [regex]::Matches(
    $uiRustText,
    'set_benefit_lot_rows\(model\(lot_rows\)\)'
).Count
if ($notificationScopeModelReplacementCount -ne 1 -or
    $notificationLotModelReplacementCount -ne 1) {
    throw 'TM-DESKTOP-NOTIFICATIONS-MODEL: Notifications must have one replacement site for each bounded model'
}
$notificationsViewText = [System.IO.File]::ReadAllText((Join-Path $uiRoot 'views\notifications-view.slint'))
$notificationsViewBoundary = $mainUiText + "`n" + $notificationsViewText + "`n" +
    $uiRustText + "`n" + $notificationsProjectionText
foreach ($requiredPattern in @(
    'if root\.notifications-visible: NotificationsView',
    '!root\.notifications-visible',
    'out property <bool> narrow:',
    'if root\.narrow:',
    'if !root\.narrow:',
    'Expiry reminders',
    'effective in-app coverage',
    'scope\.coverage-label',
    'scope\.source-label',
    'scope\.leads-label',
    'scope\.next-due-label',
    'scope\.nearest-expiry-label',
    'scope\.evidence-label',
    'scope\.warning-label',
    'lot\.kind-label',
    'lot\.quantity-label',
    'lot\.state-label',
    'lot\.expiry-label',
    'lot\.evidence-label',
    'accessible-label:.*lot\.kind-label.*lot\.quantity-label.*lot\.state-label.*lot\.expiry-label.*lot\.evidence-label'
)) {
    if ($notificationsViewBoundary -cnotmatch $requiredPattern) {
        throw "TM-DESKTOP-NOTIFICATIONS-VIEW: missing responsive Notifications contract $requiredPattern"
    }
}
foreach ($expiryVariant in @('ExactUtc', 'BoundedUtc', 'ProviderLocal', 'ProviderDate', 'Unknown')) {
    if ($uiRustText -notmatch "DesktopBenefitExpiry::$expiryVariant") {
        throw "TM-DESKTOP-NOTIFICATIONS-VIEW: missing expiry presentation $expiryVariant"
    }
}
$notificationsPublicText = @(
    [regex]::Match($notificationsProjectionText, '(?s)pub enum DesktopBenefitExpiry\s*\{.*?\r?\n\}').Value
    [regex]::Match($notificationsProjectionText, '(?s)pub struct DesktopReminderScopeRow\s*\{.*?\r?\n\}').Value
    [regex]::Match($notificationsProjectionText, '(?s)pub struct DesktopBenefitLotRow\s*\{.*?\r?\n\}').Value
    [regex]::Match($notificationsProjectionText, '(?s)pub struct DesktopNotificationsProjection\s*\{.*?\r?\n\}').Value
) -join "`n"
if ($notificationsPublicText -match '(?i)\b(?:provider|account|workspace|delivery|lot|scope|window|target|cursor|archive|credential|activation)[_-]?id\b|\b(?:absolute_)?path\b') {
    throw 'TM-DESKTOP-NOTIFICATIONS-IDENTITY: private identity or authority crossed the Notifications projection boundary'
}
$notificationsAdapterText = [regex]::Match(
    $uiRustText,
    '(?s)fn apply_notifications_projection\(.*?\r?\n\}\r?\n\r?\nfn notification_coverage_label'
).Value
if ([string]::IsNullOrWhiteSpace($notificationsAdapterText)) {
    throw 'TM-DESKTOP-NOTIFICATIONS-AUTHORITY: Notifications adapter boundary is absent'
}
$notificationsAuthorityBoundary = $notificationsProjectionText + "`n" +
    $notificationsViewText + "`n" + $notificationsAdapterText
$notificationsDeliveryPattern = '\b(?:take_notifications|acknowledge_notifications|release_notifications|BenefitReminderRuntime)\b'
$notificationsPollingPattern = '(?i)\b(?:poll_notifications|poll_reminders|Timer)\b'
$notificationsOwnerControlPattern = '(?i)\b(?:QueryService|UsageReadStore|UsageStore|Connection|rusqlite|VecDeque|HashMap|BTreeMap|LinkedList|sync_channel|notification_cache)\b|std::thread|thread::spawn|\bchannel\s*\(|callback\s+(?:activate|acknowledge|release|deliver|schedule)[A-Za-z0-9_-]*'
$notificationsDeliveryAuthorityCount = [regex]::Matches(
    $notificationsAuthorityBoundary,
    $notificationsDeliveryPattern
).Count
$notificationsPollingSurfaceCount = [regex]::Matches(
    $notificationsAuthorityBoundary,
    $notificationsPollingPattern
).Count
$notificationsOwnerControlCount = [regex]::Matches(
    $notificationsAuthorityBoundary,
    $notificationsOwnerControlPattern
).Count
if ($notificationsProjectionText -match '\.(?:opaque_id|target|delivery_id|lot_id|scope_id|account_id|workspace_id)\(' -or
    $notificationsDeliveryAuthorityCount -ne 0 -or
    $notificationsPollingSurfaceCount -ne 0 -or
    $notificationsOwnerControlCount -ne 0) {
    throw 'TM-DESKTOP-NOTIFICATIONS-AUTHORITY: Notifications route must remain read-only and delivery-receipt free'
}
if ($notificationsViewText -cnotmatch 'Text \{ text: scope\.completeness-label \+ " · " \+ scope\.evidence-label;[^\r\n]*visible: !root\.narrow;') {
    throw 'TM-DESKTOP-NOTIFICATIONS-VIEW: wide Notifications rows must preserve visible per-scope completeness'
}
$helpAboutViewPath = Join-Path $uiRoot 'views\help-about-view.slint'
$helpAboutViewText = [System.IO.File]::ReadAllText($helpAboutViewPath)
$helpAboutBoundary = $mainUiText + "`n" + $helpAboutViewText
if ($mainUiText -cnotmatch 'out property <string> help-about-layout-mode: help-view\.layout-mode;') {
    throw 'TM-DESKTOP-HELP-ABOUT-VIEW: MainWindow must expose the child content-width layout truth'
}
if ($mainUiText -cnotmatch 'out property <int> help-about-section-count: help-view\.section-count;') {
    throw 'TM-DESKTOP-HELP-ABOUT-BOUND: MainWindow must expose the child section-count truth'
}
foreach ($requiredPattern in @(
    'import \{ HelpAboutView \} from "views/help-about-view\.slint";',
    'out property <bool> help-about-visible: root\.active-route-key == "help_about";',
    'out property <string> help-about-layout-mode: help-view\.layout-mode;',
    'out property <int> help-about-section-count: help-view\.section-count;',
    'help-view := HelpAboutView',
    'visible: root\.help-about-visible;',
    '!root\.help-about-visible',
    'out property <bool> narrow: root\.width < 800px;',
    'out property <string> layout-mode: root\.narrow \? "narrow" : "wide";',
    'property <length> card-height: 232px;',
    'product-version: root\.help-product-version;'
)) {
    if ($helpAboutBoundary -cnotmatch $requiredPattern) {
        throw "TM-DESKTOP-HELP-ABOUT-VIEW: missing responsive Help About contract $requiredPattern"
    }
}
$helpAboutMountCount = [regex]::Matches(
    $mainUiText,
    '(?m)^\s*help-view := HelpAboutView\s*\{'
).Count
if ($helpAboutMountCount -ne 1 -or
    $mainUiText -match 'if root\.help-about-visible:\s*(?:[A-Za-z0-9_-]+\s*:=\s*)?HelpAboutView') {
    throw 'TM-DESKTOP-HELP-ABOUT-LIFECYCLE: Help About must stay mounted once and switch visibility only'
}
$helpAboutSectionCountMatch = [regex]::Match(
    $helpAboutViewText,
    'out property <int> section-count: ([0-9]+);'
)
$helpAboutSectionCount = if ($helpAboutSectionCountMatch.Success) {
    [int]$helpAboutSectionCountMatch.Groups[1].Value
} else {
    0
}
$helpAboutGuideCardCount = [regex]::Matches(
    $helpAboutViewText,
    '(?m)^\s*HelpSectionCard\s*\{'
).Count
$helpAboutAttributionCardCount = [regex]::Matches(
    $helpAboutViewText,
    '(?m)^\s*AttributionCard\s*\{'
).Count
$helpAboutRenderedSectionCount = $helpAboutGuideCardCount + $helpAboutAttributionCardCount
if ($helpAboutSectionCount -ne 6 -or
    $helpAboutGuideCardCount -ne 5 -or
    $helpAboutAttributionCardCount -ne 1 -or
    $helpAboutRenderedSectionCount -ne $helpAboutSectionCount -or
    [regex]::Matches($helpAboutViewText, 'out property <int> section-count:').Count -ne 1) {
    throw 'TM-DESKTOP-HELP-ABOUT-BOUND: Help About must expose exactly six fixed sections'
}
$helpAboutAttributionCount = [regex]::Matches($helpAboutViewText, '\bAboutSlint\s*\{').Count
$helpAboutAttributionImportCount = [regex]::Matches(
    $helpAboutViewText,
    'import \{ AboutSlint, ScrollView \} from "std-widgets\.slint";'
).Count
$helpAboutAttributionHeightCount = [regex]::Matches(
    $helpAboutViewText,
    'AboutSlint\s*\{\s*height: 112px;'
).Count
$helpAboutAttributionTextSizeCount = [regex]::Matches(
    $helpAboutViewText,
    '(?s)text: "WhereMyTokens and ccusage are pinned external MIT references, not runtime dependencies\.";\s*color:[^;]+;\s*font-size: 10px;'
).Count
if ($helpAboutAttributionCount -ne 1 -or
    $helpAboutAttributionImportCount -ne 1 -or
    $helpAboutAttributionHeightCount -ne 1 -or
    $helpAboutAttributionTextSizeCount -ne 1) {
    throw 'TM-DESKTOP-HELP-ABOUT-ATTRIBUTION: Help About must mount exactly one standard Slint attribution widget'
}
foreach ($requiredText in @(
    'Start here',
    'Data sources and truth',
    'Privacy by design',
    'Health and recovery',
    'Automation status',
    'About and licenses',
    'No prompts, responses, reasoning, commands',
    'CLI and stdio MCP are not available',
    'No browser session reuse or private endpoint replay',
    'Data Health owns backup, verification, restore, rebuild, and recovery truth. Settings owns backup policy and portable configuration.',
    'TokenMaster · MIT',
    $fixedUpstreamAttribution
)) {
    if (-not $helpAboutViewText.Contains($requiredText, [System.StringComparison]::Ordinal)) {
        throw "TM-DESKTOP-HELP-ABOUT-CONTENT: missing truthful Help About content $requiredText"
    }
}
$helpAboutAccessibleRegionCount = [regex]::Matches(
    $helpAboutViewText,
    'accessible-role:\s*region;'
).Count
if ($helpAboutAccessibleRegionCount -ne 4) {
    throw 'TM-DESKTOP-HELP-ABOUT-VIEW: Help About accessible region structure drifted'
}
$helpAboutVersionSetterPattern = 'set_help_product_version\(env!\("CARGO_PKG_VERSION"\)\.into\(\)\)'
$helpAboutVersionSetterCount = [regex]::Matches(
    $uiRustText,
    $helpAboutVersionSetterPattern
).Count
$helpAboutConstructor = [regex]::Match(
    $uiRustText,
    '(?s)pub fn new_with_reliable_state_and_session_sink\(.*?\r?\n    \}\r?\n\r?\n    #\[must_use\]'
).Value
if ($helpAboutVersionSetterCount -ne 1 -or
    [regex]::Matches($helpAboutConstructor, $helpAboutVersionSetterPattern).Count -ne 1 -or
    $uiRustText -match 'std::env::var|option_env!|git describe') {
    throw 'TM-DESKTOP-HELP-ABOUT-VERSION: Help About version must be applied exactly once from the compile-time package version'
}
$helpAboutModelPattern = '(?i)\b(?:ModelRc|VecModel|model\s*<)\b|property\s*<\[|(?m)^\s*for\s+[A-Za-z0-9_-]+\s+in\s+'
$helpAboutAuthorityPattern = '(?i)\bcallback\b|Platform\.open-url|https?://|\b(?:QueryService|UsageReadStore|UsageStore|Connection|rusqlite|reqwest|webbrowser)\b|std::(?:env|fs|net|process)|\b(?:activate|acknowledge|deliver|schedule)-benefit\b'
$helpAboutPollingPattern = '(?i)\b(?:Timer|poll_help|poll_about|thread::spawn|thread::sleep)\b'
$helpAboutModelCount = [regex]::Matches($helpAboutViewText, $helpAboutModelPattern).Count
$helpAboutAuthorityCount = [regex]::Matches($helpAboutViewText, $helpAboutAuthorityPattern).Count
$helpAboutPollingSurfaceCount = [regex]::Matches($helpAboutViewText, $helpAboutPollingPattern).Count
if ($helpAboutAuthorityCount -ne 0) {
    throw 'TM-DESKTOP-HELP-ABOUT-AUTHORITY: Help About must remain static and control-free'
}
if ($helpAboutModelCount -ne 0 -or $helpAboutPollingSurfaceCount -ne 0) {
    throw 'TM-DESKTOP-HELP-ABOUT-BOUND: Help About must not own models timers or polling'
}
$helpAboutFalseClaimPattern = '(?i)\b(?:release (?:accepted|ready|complete)|package (?:signed|ready)|signed (?:build|package|release)|SBOM (?:included|available|complete)|MSVC (?:build|release) (?:available|complete)|CLI is available|stdio MCP is available|automation is available|all providers (?:are )?(?:supported|available)|every provider (?:is )?(?:supported|available))\b'
if ($helpAboutViewText -match $helpAboutFalseClaimPattern) {
    throw 'TM-DESKTOP-HELP-ABOUT-CLAIM: Help About must not claim deferred release or automation capability'
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
        model_row_maximum = 64
        models_model_replacement_count = $modelsModelReplacementCount
        models_projection_application_count = $modelsProjectionCallCount - 1
        analytics_query_call_count = $analyticsQueryCallCount
        models_polling_surface_count = 0
        project_row_maximum = 32
        projects_model_replacement_count = $projectsModelReplacementCount
        projects_projection_application_count = $projectsProjectionCallCount - 1
        git_query_call_count = $gitQueryCallCount
        projects_polling_surface_count = 0
        activity_row_maximum = 12
        activity_model_replacement_count = $activityModelReplacementCount
        activity_projection_application_count = $activityProjectionCallCount - 1
        activity_query_call_count = $activityQueryCallCount
        activity_polling_surface_count = 0
        notification_scope_maximum = $notificationBounds.MAX_NOTIFICATION_SCOPES
        notification_lot_maximum = $notificationBounds.MAX_NOTIFICATION_LOTS
        notification_lead_maximum = $notificationBounds.MAX_NOTIFICATION_LEADS
        notification_scope_model_replacement_count = $notificationScopeModelReplacementCount
        notification_lot_model_replacement_count = $notificationLotModelReplacementCount
        notifications_projection_application_count = $notificationsProjectionCallCount - 1
        benefit_query_call_count = $benefitQueryCallCount
        notifications_delivery_authority_count = $notificationsDeliveryAuthorityCount
        notifications_owner_control_count = $notificationsOwnerControlCount
        notifications_polling_surface_count = $notificationsPollingSurfaceCount
        in_app_notification_row_maximum = 256
        in_app_notification_model_count = $inAppModelCount
        in_app_notification_presented_after_apply_count = $successfulApplyCount
        in_app_notification_ready_before_receipt_count = $readyBeforeReceiptCount
        in_app_notification_accessible_label_count = 1
        in_app_notification_epoch_guard_count = $inAppEpochGuardCount
        help_about_section_count = $helpAboutRenderedSectionCount
        help_about_version_setter_count = $helpAboutVersionSetterCount
        help_about_slint_attribution_count = $helpAboutAttributionCount
        help_about_model_count = $helpAboutModelCount
        help_about_authority_count = $helpAboutAuthorityCount
        help_about_polling_surface_count = $helpAboutPollingSurfaceCount
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
    model_row_maximum = 64
    models_model_replacement_count = $modelsModelReplacementCount
    models_projection_application_count = $modelsProjectionCallCount - 1
    analytics_query_call_count = $analyticsQueryCallCount
    models_polling_surface_count = 0
    project_row_maximum = 32
    projects_model_replacement_count = $projectsModelReplacementCount
    projects_projection_application_count = $projectsProjectionCallCount - 1
    git_query_call_count = $gitQueryCallCount
    projects_polling_surface_count = 0
    activity_row_maximum = 12
    activity_model_replacement_count = $activityModelReplacementCount
    activity_projection_application_count = $activityProjectionCallCount - 1
    activity_query_call_count = $activityQueryCallCount
    activity_polling_surface_count = 0
    notification_scope_maximum = $notificationBounds.MAX_NOTIFICATION_SCOPES
    notification_lot_maximum = $notificationBounds.MAX_NOTIFICATION_LOTS
    notification_lead_maximum = $notificationBounds.MAX_NOTIFICATION_LEADS
    notification_scope_model_replacement_count = $notificationScopeModelReplacementCount
    notification_lot_model_replacement_count = $notificationLotModelReplacementCount
    notifications_projection_application_count = $notificationsProjectionCallCount - 1
    benefit_query_call_count = $benefitQueryCallCount
    notifications_delivery_authority_count = $notificationsDeliveryAuthorityCount
    notifications_owner_control_count = $notificationsOwnerControlCount
    notifications_polling_surface_count = $notificationsPollingSurfaceCount
    in_app_notification_row_maximum = 256
    in_app_notification_model_count = $inAppModelCount
    in_app_notification_presented_after_apply_count = $successfulApplyCount
    in_app_notification_ready_before_receipt_count = $readyBeforeReceiptCount
    in_app_notification_accessible_label_count = 1
    in_app_notification_epoch_guard_count = $inAppEpochGuardCount
    help_about_section_count = $helpAboutRenderedSectionCount
    help_about_version_setter_count = $helpAboutVersionSetterCount
    help_about_slint_attribution_count = $helpAboutAttributionCount
    help_about_model_count = $helpAboutModelCount
    help_about_authority_count = $helpAboutAuthorityCount
    help_about_polling_surface_count = $helpAboutPollingSurfaceCount
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
