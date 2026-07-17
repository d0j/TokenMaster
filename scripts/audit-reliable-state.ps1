[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$RepositoryRoot,
    [switch]$SourceOnly
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$rootManifest = Join-Path $root 'Cargo.toml'
$stateRoot = Join-Path $root 'crates\state'
$stateManifest = Join-Path $stateRoot 'Cargo.toml'
$stateSource = Join-Path $stateRoot 'src'

foreach ($required in @($rootManifest, $stateManifest, $stateSource)) {
    if (-not (Test-Path -LiteralPath $required)) {
        throw "TM-STATE-MISSING-BOUNDARY: $([System.IO.Path]::GetFileName($required))"
    }
}

$rootManifestText = [System.IO.File]::ReadAllText($rootManifest)
$workspaceSection = [regex]::Match(
    $rootManifestText,
    '(?ms)^\s*\[workspace\]\s*$\s*(?<body>.*?)(?=^\s*\[|\z)'
)
$workspaceMembers = if ($workspaceSection.Success) {
    [regex]::Match(
        $workspaceSection.Groups['body'].Value,
        '(?ms)^\s*members\s*=\s*\[(?<items>.*?)\]'
    )
} else {
    [System.Text.RegularExpressions.Match]::Empty
}
$workspaceMemberNames = if ($workspaceMembers.Success) {
    $memberItems = [regex]::Replace(
        $workspaceMembers.Groups['items'].Value,
        '(?m)#.*$',
        ''
    )
    @(
        [regex]::Matches($memberItems, '"(?<member>[^"\r\n]+)"') |
            ForEach-Object { $_.Groups['member'].Value }
    )
} else {
    @()
}
$stateWorkspaceMembers = @($workspaceMemberNames | Where-Object { $_ -eq 'crates/state' })
if ($stateWorkspaceMembers.Count -ne 1) {
    throw 'TM-STATE-WORKSPACE: crates/state must be one exact workspace member'
}

$manifestText = [System.IO.File]::ReadAllText($stateManifest)
if ($manifestText -notmatch '(?m)^name\s*=\s*"tokenmaster-state"\s*$') {
    throw 'TM-STATE-PACKAGE: package identity must be tokenmaster-state'
}

$mainSource = Join-Path $stateSource 'main.rs'
$binarySource = Join-Path $stateSource 'bin'
if ($manifestText -match '\[\[bin\]\]' -or
    $manifestText -match '(?m)^\s*autobins\s*=\s*true\s*$' -or
    (Test-Path -LiteralPath $mainSource) -or
    (Test-Path -LiteralPath $binarySource)) {
    throw 'TM-STATE-BINARY-TARGET: tokenmaster-state must remain library-only'
}
if ($manifestText -match '(?m)^\s*build\s*=' -or
    (Test-Path -LiteralPath (Join-Path $stateRoot 'build.rs'))) {
    throw 'TM-STATE-BUILD-SCRIPT: tokenmaster-state must not own a build script'
}

$dependencyNames = [System.Collections.Generic.List[string]]::new()
$insideDependencies = $false
foreach ($line in ($manifestText -split "`r?`n")) {
    if ($line -match '^\s*\[dependencies\]\s*$') {
        $insideDependencies = $true
        continue
    }
    if ($line -match '^\s*\[') {
        $insideDependencies = $false
        continue
    }
    if ($insideDependencies -and $line -match '^\s*([A-Za-z0-9_-]+)(?:\.[A-Za-z0-9_-]+)?\s*=') {
        $dependencyNames.Add($Matches[1])
    }
}
$directProductionDependencies = @($dependencyNames | Sort-Object -Unique)
$expectedDependencies = @(
    'serde', 'serde_json', 'sha2', 'thiserror', 'tokenmaster-platform'
)
if ($directProductionDependencies.Count -ne $expectedDependencies.Count -or
    @($expectedDependencies | Where-Object { $_ -notin $directProductionDependencies }).Count -ne 0) {
    throw "TM-STATE-DEPENDENCIES: direct dependency set drifted: $($directProductionDependencies -join ', ')"
}

$testOnlySource = Join-Path $stateSource 'record_contract_tests.rs'
$librarySource = Join-Path $stateSource 'lib.rs'
if (-not (Test-Path -LiteralPath $testOnlySource)) {
    throw 'TM-STATE-TEST-BOUNDARY: record contract test module is missing'
}
$librarySourceText = [System.IO.File]::ReadAllText($librarySource)
$testModulePattern = '(?m)^#\[cfg\(test\)\]\s*\r?\nmod record_contract_tests;\s*$'
if (@([regex]::Matches($librarySourceText, $testModulePattern)).Count -ne 1 -or
    @([regex]::Matches($librarySourceText, '\brecord_contract_tests\b')).Count -ne 1) {
    throw 'TM-STATE-TEST-BOUNDARY: record contract code must remain one cfg(test)-only module'
}
$rustFiles = @(
    Get-ChildItem -LiteralPath $stateSource -Recurse -File -Filter '*.rs' |
        Where-Object { $_.FullName -ne $testOnlySource }
)
if ($rustFiles.Count -eq 0) {
    throw 'TM-STATE-SOURCE: tokenmaster-state has no Rust library source'
}
$productionText = ($rustFiles | ForEach-Object {
    [System.IO.File]::ReadAllText($_.FullName)
}) -join "`n"

$approvedStdIoPattern = '(?m)^\s*use\s+std\s*::\s*io\s*::\s*\{\s*self\s*,\s*Write\s*,?\s*\}\s*;\s*$'
$approvedPlatformPattern = '(?ms)^\s*use\s+tokenmaster_platform\s*::\s*\{\s*DurableFileError\s*,\s*DurableFileTarget\s*,\s*DurableStagedFile\s*,\s*MAX_DURABLE_WRITE_CHUNK_BYTES\s*,\s*ValidatedLocalDirectory\s*,?\s*\}\s*;\s*$'
$approvedStdIoImports = @([regex]::Matches($productionText, $approvedStdIoPattern))
$approvedPlatformImports = @([regex]::Matches($productionText, $approvedPlatformPattern))
if ($approvedStdIoImports.Count -ne 1 -or $approvedPlatformImports.Count -ne 1) {
    throw 'TM-STATE-APPROVED-IO: exact bounded JSON and durable-file imports must each appear once'
}
$approvedIoMembers = @('Error', 'ErrorKind', 'Result')
$ioMemberUses = @([regex]::Matches($productionText, '\bio::(?<member>[A-Za-z_][A-Za-z0-9_]*)'))
$unapprovedIoMembers = @(
    $ioMemberUses |
        Where-Object { $_.Groups['member'].Value -notin $approvedIoMembers }
)
if ($unapprovedIoMembers.Count -ne 0) {
    throw 'TM-STATE-APPROVED-IO: std::io use exceeds bounded writer error/result authority'
}
$exactChildUses = @([regex]::Matches($productionText, '\bexact_child\b'))
$approvedExactChildPattern = 'DurableFileTarget\s*::\s*exact_child\s*\(\s*directory\s*,\s*"(?<child>settings-a\.tms|settings-b\.tms|run-a\.tms|run-b\.tms|recovery-a\.tms|recovery-b\.tms)"\s*\)'
$approvedExactChildUses = @([regex]::Matches($productionText, $approvedExactChildPattern))
$expectedRecordChildren = @(
    'settings-a.tms', 'settings-b.tms', 'run-a.tms', 'run-b.tms',
    'recovery-a.tms', 'recovery-b.tms'
)
$approvedRecordChildren = @(
    $approvedExactChildUses |
        ForEach-Object { $_.Groups['child'].Value } |
        Sort-Object -Unique
)
if ($exactChildUses.Count -ne 6 -or
    $approvedExactChildUses.Count -ne 6 -or
    $approvedRecordChildren.Count -ne 6 -or
    @($expectedRecordChildren | Where-Object { $_ -notin $approvedRecordChildren }).Count -ne 0) {
    throw 'TM-STATE-EXACT-CHILD: state may construct only the six fixed record slots'
}
$authorityText = [regex]::Replace($productionText, $approvedStdIoPattern, '')
$authorityText = [regex]::Replace($authorityText, $approvedPlatformPattern, '')

$publicPathPattern = '(?s)\bpub(?:\([^)]*\))?\s+(?:(?:const|async|unsafe)\s+)*fn\s+\w+[^;{]*(?:std::path::)?(?:Path|PathBuf)\b[^;{]*[;{]'
if ($productionText -match $publicPathPattern) {
    throw 'TM-STATE-ARBITRARY-PATH: public state API must not accept filesystem paths'
}
$publicRecordAuthorityPattern = '(?m)^\s*pub\s+(?:use\s+record\b|mod\s+record\b|struct\s+RedundantRecordStore\b)'
if ($productionText -match $publicRecordAuthorityPattern) {
    throw 'TM-STATE-RECORD-VISIBILITY: generic record filesystem authority must remain crate-private'
}
$forbiddenAuthorityPattern = '(?s)https?://|\bstd\b|\btokenmaster_platform\b|\bmacro_rules\s*!|\b(?:Command|TcpStream|TcpListener|UdpSocket)\b|\b(?:slint|rusqlite|tokio|reqwest|ureq|webbrowser|headless_chrome|zip|tar)::|\b(?:SELECT|INSERT|UPDATE|DELETE\s+FROM|PRAGMA)\b|\b(?:include|include_str|include_bytes)!\s*\(|#\s*\[\s*path\s*=|powershell(?:\.exe)?|cmd(?:\.exe)?|bash(?:\.exe)?|\bsh\s+-c\b|\bAuthorization\b|\bBearer\s'
if ($authorityText -cmatch $forbiddenAuthorityPattern) {
    throw 'TM-STATE-FORBIDDEN-AUTHORITY: state source contains standard-library/platform/macro/filesystem/network/shell/process/SQL/UI/archive/external-source authority'
}

if ($SourceOnly) {
    [ordered]@{
        result = 'pass'
        scope = 'source-only'
        package = 'tokenmaster-state'
        workspace_member_count = $stateWorkspaceMembers.Count
        binary_target_count = 0
        direct_production_dependency_count = $directProductionDependencies.Count
        rust_source_file_count = $rustFiles.Count
        approved_std_io_import_count = $approvedStdIoImports.Count
        approved_platform_import_count = $approvedPlatformImports.Count
        forbidden_authority_count = 0
        arbitrary_path_constructor_count = 0
    } | ConvertTo-Json -Compress
    return
}

$cargo = (Get-Command cargo.exe -CommandType Application -ErrorAction Stop).Source
$metadataJson = & $cargo +1.97.0 metadata --locked --format-version 1 --manifest-path $rootManifest
if ($LASTEXITCODE -ne 0) {
    throw 'TM-STATE-METADATA: cargo metadata failed'
}
$metadata = $metadataJson | ConvertFrom-Json -Depth 100
$statePackages = @($metadata.packages | Where-Object { $_.name -eq 'tokenmaster-state' })
if ($statePackages.Count -ne 1) {
    throw 'TM-STATE-PACKAGE: tokenmaster-state must resolve exactly once'
}
$metadataStateMembers = @(
    $metadata.workspace_members |
        Where-Object { [string]$_ -eq [string]$statePackages[0].id }
)
if ($metadataStateMembers.Count -ne 1) {
    throw 'TM-STATE-WORKSPACE: tokenmaster-state must resolve as one exact workspace member'
}
$metadataDependencies = @(
    $statePackages[0].dependencies |
        Where-Object { $null -eq $_.kind } |
        ForEach-Object { $_.name } |
        Sort-Object -Unique
)
if ($metadataDependencies.Count -ne $expectedDependencies.Count -or
    @($expectedDependencies | Where-Object { $_ -notin $metadataDependencies }).Count -ne 0) {
    throw "TM-STATE-DEPENDENCIES: metadata dependency set drifted: $($metadataDependencies -join ', ')"
}
$binaryTargets = @($statePackages[0].targets | Where-Object { $_.kind -contains 'bin' })
if ($binaryTargets.Count -ne 0) {
    throw 'TM-STATE-BINARY-TARGET: metadata contains a state binary target'
}

$treeText = (& $cargo +1.97.0 tree -p tokenmaster-state -e normal --prefix none --format '{p}' --manifest-path $rootManifest) -join "`n"
if ($LASTEXITCODE -ne 0) {
    throw 'TM-STATE-TREE: cargo dependency tree failed'
}
if ($treeText -match '(?m)^(?:zip|tar|tokio|reqwest|ureq|slint|webbrowser|headless_chrome)\s+v') {
    throw 'TM-STATE-TRANSITIVE-AUTHORITY: forbidden dependency entered the state tree'
}

[ordered]@{
    result = 'pass'
    scope = 'workspace'
    package = 'tokenmaster-state'
    workspace_member_count = $metadataStateMembers.Count
    binary_target_count = $binaryTargets.Count
    direct_production_dependencies = $metadataDependencies
    direct_production_dependency_count = $metadataDependencies.Count
    rust_source_file_count = $rustFiles.Count
    approved_std_io_import_count = $approvedStdIoImports.Count
    approved_platform_import_count = $approvedPlatformImports.Count
    forbidden_authority_count = 0
    arbitrary_path_constructor_count = 0
    forbidden_transitive_dependency_count = 0
} | ConvertTo-Json -Compress
