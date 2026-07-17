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
if ($rustFiles.Count -ne 6 -or $uiFiles.Count -ne 5) {
    throw 'TM-DESKTOP-FILE-COUNT: production desktop boundary must contain six Rust and five Slint files'
}
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
$eventScheduleCount = [regex]::Matches($bridgeText, 'slint::invoke_from_event_loop\(').Count
if ($eventScheduleCount -ne 1) {
    throw 'TM-DESKTOP-BRIDGE-EVENT: desktop bridge must contain exactly one event-loop scheduling site'
}
if ($bridgeText -notmatch 'window:\s*slint::Weak<MainWindow>') {
    throw 'TM-DESKTOP-BRIDGE-WEAK: desktop bridge must retain only a weak Slint window handle'
}
if ($bridgeText -match 'window:\s*MainWindow|\b(slint::Timer|std::thread|thread::spawn|thread::sleep)\b') {
    throw 'TM-DESKTOP-BRIDGE-POLLING: desktop bridge must not retain a strong window, timer, or polling thread'
}
$uiAdapterText = [System.IO.File]::ReadAllText((Join-Path $sourceRoot 'ui.rs')) + "`n" +
    (($uiFiles | ForEach-Object { [System.IO.File]::ReadAllText($_.FullName) }) -join "`n")
if ($uiAdapterText -match 'QueryService::|RefreshWorker::|DesktopController::|\.usage_analytics\(') {
    throw 'TM-DESKTOP-UI-QUERY: Slint callbacks must not perform controller or query work'
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
if ($productionText -match '\b(QuotaRow|SessionRow|ChartPoint|quota-targets|session-rows|chart-points)\b') {
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
        bridge_polling_surface_count = 0
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
    'anyhow', 'slint', 'tokenmaster-engine', 'tokenmaster-product', 'tokenmaster-query'
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
    bridge_polling_surface_count = 0
    mock_data_model_count = 0
    direct_authority_dependency_count = 0
    forbidden_source_authority_count = 0
    femtovg_feature_count = 0
    probe_dependency_count = 0
    release_artifact_count = 0
} | ConvertTo-Json -Compress
