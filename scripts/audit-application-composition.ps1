[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$RepositoryRoot,
    [switch]$SourceOnly
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$rootManifest = Join-Path $root 'Cargo.toml'
$appRoot = Join-Path $root 'crates\app'
$appManifest = Join-Path $appRoot 'Cargo.toml'
$appSource = Join-Path $appRoot 'src'
$desktopManifest = Join-Path $root 'crates\desktop\Cargo.toml'

foreach ($required in @($rootManifest, $appManifest, $appSource, $desktopManifest)) {
    if (-not (Test-Path -LiteralPath $required)) {
        throw "TM-APP-MISSING-BOUNDARY: $([System.IO.Path]::GetFileName($required))"
    }
}

$manifestText = [System.IO.File]::ReadAllText($appManifest)
$desktopManifestText = [System.IO.File]::ReadAllText($desktopManifest)
if ([regex]::Matches($manifestText, '\[\[bin\]\]').Count -ne 1 -or
    $manifestText -notmatch 'name\s*=\s*"TokenMaster"' -or
    $manifestText -notmatch 'path\s*=\s*"src/main\.rs"') {
    throw 'TM-APP-BINARY-OWNER: tokenmaster-app must declare the sole TokenMaster binary'
}
if ($desktopManifestText -match '\[\[bin\]\]|name\s*=\s*"TokenMaster"') {
    throw 'TM-APP-DUPLICATE-BINARY: tokenmaster-desktop must remain library-only'
}
if ($manifestText -match '\btokenmaster-m0\b|[\\/]probe-app\b|\brenderer-femtovg\b') {
    throw 'TM-APP-PROBE-DEPENDENCY: production composition must not depend on the M0 probe'
}

$rustFiles = @(
    Get-ChildItem -LiteralPath $appSource -Recurse -File -Filter '*.rs' |
        Where-Object { $_.Name -notlike '*_tests.rs' }
)
if ($rustFiles.Count -ne 4) {
    throw 'TM-APP-FILE-COUNT: application composition must contain exactly four Rust files'
}
$productionText = ($rustFiles | ForEach-Object {
    [System.IO.File]::ReadAllText($_.FullName)
}) -join "`n"
$applicationText = [System.IO.File]::ReadAllText((Join-Path $appSource 'application.rs'))
$dataRootText = [System.IO.File]::ReadAllText((Join-Path $appSource 'data_root.rs'))

foreach ($contract in @(
    @{ Name = 'TM-APP-LIVE-OWNER'; Pattern = 'LiveRuntime::start_notified\('; Count = 1 },
    @{ Name = 'TM-APP-QUOTA-OWNER'; Pattern = 'CodexQuotaRuntime::start_notified\('; Count = 1 },
    @{ Name = 'TM-APP-REMINDER-OWNER'; Pattern = 'BenefitReminderRuntime::start_notified\('; Count = 1 },
    @{ Name = 'TM-APP-CONTROLLER'; Pattern = 'DesktopController::open\('; Count = 1 },
    @{ Name = 'TM-APP-BRIDGE'; Pattern = '\.snapshot_bridge\('; Count = 1 },
    @{ Name = 'TM-APP-EVENT-LOOP'; Pattern = 'slint::run_event_loop\('; Count = 1 },
    @{ Name = 'TM-APP-PORTABLE-MARKER'; Pattern = '"tokenmaster\.portable"'; Count = 1 },
    @{ Name = 'TM-APP-ARCHIVE-NAME'; Pattern = '"tokenmaster\.sqlite3"'; Count = 1 }
)) {
    $actual = [regex]::Matches($productionText, $contract.Pattern).Count
    if ($actual -ne $contract.Count) {
        throw "$($contract.Name): expected $($contract.Count), observed $actual"
    }
}

if ($applicationText -notmatch 'Weak<Mutex<Option<ApplicationBundle>>>' -or
    $applicationText -notmatch 'impl WorkerCompletionNotifier for ApplicationRuntimeNotifier') {
    throw 'TM-APP-WEAK-NOTIFIER: runtime completion notifier must retain only weak application state'
}
if ($applicationText -match '\b(slint::Timer|std::thread|thread::spawn|thread::sleep)\b') {
    throw 'TM-APP-POLLING: application composition must not add a timer or polling thread'
}
if ($productionText -match '\bstd::env::(args|args_os|current_dir|set_current_dir)\b') {
    throw 'TM-APP-ARBITRARY-ROOT: command-line or working-directory data roots are forbidden'
}
$environmentNames = @(
    [regex]::Matches($dataRootText, 'var_os\("([A-Z_]+)"\)') |
        ForEach-Object { $_.Groups[1].Value } |
        Sort-Object -Unique
)
$expectedEnvironmentNames = @('CODEX_HOME', 'LOCALAPPDATA', 'USERPROFILE')
if ($environmentNames.Count -ne $expectedEnvironmentNames.Count -or
    @($expectedEnvironmentNames | Where-Object { $_ -notin $environmentNames }).Count -ne 0) {
    throw "TM-APP-ARBITRARY-ROOT: environment surface drifted: $($environmentNames -join ', ')"
}
if ($productionText -match 'https?://|\b(Command|TcpStream|TcpListener|UdpSocket)\b|\b(rusqlite|notify|reqwest|ureq|webbrowser|headless_chrome)\b|\b(SELECT|INSERT|UPDATE|DELETE\s+FROM|PRAGMA)\b|powershell(?:\.exe)?|cmd(?:\.exe)?|bash(?:\.exe)?|\bsh\s+-c\b|\bAuthorization\b|\bBearer\s') {
    throw 'TM-APP-FORBIDDEN-AUTHORITY: composition contains network/shell/SQL/browser/credential authority'
}
if ($productionText -match '\b(WhereMyTokens|WhereMyToken|WhereMyTokensGo|ccusage-go)\b') {
    throw 'TM-APP-OLD-PROJECT: production composition contains an old project identity'
}
if ($dataRootText -notmatch 'ValidatedLocalDirectory::new' -or
    $dataRootText -notmatch 'fs::create_dir\(' -or
    $dataRootText -match 'create_dir_all|\.join\("portable"\)') {
    throw 'TM-APP-DATA-ROOT: exact one-child validated data-root policy drifted'
}

if ($SourceOnly) {
    [ordered]@{
        result = 'pass'
        scope = 'source-only'
        rust_source_file_count = $rustFiles.Count
        production_binary_owner_count = 1
        live_runtime_owner_count = 1
        quota_runtime_owner_count = 1
        reminder_runtime_owner_count = 1
        desktop_controller_count = 1
        desktop_bridge_count = 1
        application_polling_surface_count = 0
        arbitrary_root_surface_count = 0
    } | ConvertTo-Json -Compress
    return
}

$metadataJson = & cargo +1.97.0 metadata --locked --format-version 1 --manifest-path $rootManifest
if ($LASTEXITCODE -ne 0) {
    throw 'TM-APP-METADATA: cargo metadata failed'
}
$metadata = $metadataJson | ConvertFrom-Json -Depth 100
$appPackages = @($metadata.packages | Where-Object { $_.name -eq 'tokenmaster-app' })
if ($appPackages.Count -ne 1) {
    throw 'TM-APP-PACKAGE: tokenmaster-app must resolve exactly once'
}
$directProductionDependencies = @(
    $appPackages[0].dependencies |
        Where-Object { $null -eq $_.kind } |
        ForEach-Object { $_.name } |
        Sort-Object -Unique
)
$expectedDependencies = @(
    'slint', 'tokenmaster-codex', 'tokenmaster-desktop', 'tokenmaster-engine',
    'tokenmaster-platform', 'tokenmaster-product', 'tokenmaster-runtime'
)
if ($directProductionDependencies.Count -ne $expectedDependencies.Count -or
    @($expectedDependencies | Where-Object { $_ -notin $directProductionDependencies }).Count -ne 0) {
    throw "TM-APP-DEPENDENCIES: direct dependency set drifted: $($directProductionDependencies -join ', ')"
}
$tokenMasterTargets = @(
    $metadata.packages | ForEach-Object {
        $package = $_
        $_.targets | Where-Object { $_.kind -contains 'bin' -and $_.name -eq 'TokenMaster' } |
            ForEach-Object { [pscustomobject]@{ Package = $package.name; Source = $_.src_path } }
    }
)
if ($tokenMasterTargets.Count -ne 1 -or $tokenMasterTargets[0].Package -ne 'tokenmaster-app') {
    throw 'TM-APP-DUPLICATE-BINARY: exactly one TokenMaster target must be owned by tokenmaster-app'
}

$featureTree = (& cargo +1.97.0 tree -p tokenmaster-app -e features --manifest-path $rootManifest) -join "`n"
if ($LASTEXITCODE -ne 0) {
    throw 'TM-APP-TREE: cargo feature tree failed'
}
if ($featureTree -notmatch 'renderer-software' -or $featureTree -match 'renderer-femtovg|tokenmaster-m0') {
    throw 'TM-APP-RENDERER: production tree must contain software renderer and no probe/FemtoVG'
}

& cargo +1.97.0 build --release --locked --manifest-path $rootManifest -p tokenmaster-app
if ($LASTEXITCODE -ne 0) {
    throw 'TM-APP-BUILD: release application build failed'
}
$targetDirectory = [System.IO.Path]::GetFullPath([string]$metadata.target_directory)
$artifacts = @(
    Get-ChildItem -LiteralPath $targetDirectory -Recurse -File -Filter 'TokenMaster.exe' |
        Where-Object { $_.FullName -match '[\\/]release[\\/]TokenMaster\.exe$' }
)
if ($artifacts.Count -ne 1) {
    throw 'TM-APP-ARTIFACT: release TokenMaster executable was not found'
}
$artifact = $artifacts[0].FullName
$artifactText = [System.Text.Encoding]::ASCII.GetString(
    [System.IO.File]::ReadAllBytes($artifact)
)
foreach ($needle in @(
    'seed_probe_models', 'TokenMaster M0', 'demo-session-', 'WhereMyTokens',
    'PRIVATE_GIT_RUNTIME_REPOSITORY', 'PRIVATE_SESSION_NAME.jsonl',
    'PIPELINE_PRIVATE_SENTINEL_91A7', 'PRIVATE_PARENT_MARKER',
    'Private@Example.com', 'credit_private_76e5', 'C:\private\codex-home',
    'Authorization: Bearer', 'auth.json'
)) {
    if ($artifactText.IndexOf($needle, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "TM-APP-BINARY-STRING: release executable contains forbidden string: $needle"
    }
}

[ordered]@{
    result = 'pass'
    package = 'tokenmaster-app'
    binary = 'TokenMaster.exe'
    direct_production_dependencies = $directProductionDependencies
    rust_source_file_count = $rustFiles.Count
    production_binary_owner_count = 1
    live_runtime_owner_count = 1
    quota_runtime_owner_count = 1
    reminder_runtime_owner_count = 1
    desktop_controller_count = 1
    desktop_bridge_count = 1
    application_polling_surface_count = 0
    arbitrary_root_surface_count = 0
    femtovg_feature_count = 0
    probe_dependency_count = 0
    release_artifact_count = 1
    forbidden_binary_string_count = 0
} | ConvertTo-Json -Compress
