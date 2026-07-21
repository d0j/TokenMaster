[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$RepositoryRoot,
    [switch]$SourceOnly
)

$ErrorActionPreference = 'Stop'
$expectedDependencyPolicySha256 = '26a334260c1739469857836d52d8fcc7719cb9d14319413fd885d620799dc3cc'
$expectedFeaturePolicySha256 = '9b030a99fe87f68c2aecb2f728121fe655cca6c5818e4295873b223f685f5be8'
$root = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$rootManifest = Join-Path $root 'Cargo.toml'
$stateManifest = Join-Path $root 'crates\state\Cargo.toml'
$packageSource = Join-Path $root 'crates\state\src\package'
$upstreamManifest = Join-Path $root 'third_party\UPSTREAM.toml'
$licenseFiles = @(
    (Join-Path $root 'third_party\licenses\WhereMyTokens-MIT.txt'),
    (Join-Path $root 'third_party\licenses\ccusage-MIT.txt')
)
$coverageFiles = [ordered]@{
    fault = Join-Path $root 'crates\state\tests\fault_matrix_contract.rs'
    package = Join-Path $root 'crates\state\tests\package_adversarial_contract.rs'
    encryption = Join-Path $root 'crates\state\tests\encryption_contract.rs'
    backup = Join-Path $root 'crates\store\tests\backup_adversarial_contract.rs'
    platform = Join-Path $root 'crates\platform\tests\archive_recovery_contract.rs'
    app = Join-Path $root 'crates\app\tests\recovery_adversarial_contract.rs'
}
$privacySurfaceFiles = @(
    (Join-Path $root 'crates\state\src\error.rs'),
    (Join-Path $root 'crates\app\src\state.rs'),
    (Join-Path $root 'crates\desktop\src\reliable_state.rs')
)

foreach ($required in @(
    $rootManifest,
    $stateManifest,
    $packageSource,
    $upstreamManifest
) + $licenseFiles + @($coverageFiles.Values) + $privacySurfaceFiles) {
    if (-not (Test-Path -LiteralPath $required)) {
        throw "TM-BACKUP-MISSING-BOUNDARY: $([System.IO.Path]::GetFileName($required))"
    }
}

$packageFiles = @(
    Get-ChildItem -LiteralPath $packageSource -Recurse -File -Filter '*.rs' |
        Sort-Object FullName
)
if ($packageFiles.Count -ne 7) {
    throw "TM-BACKUP-PACKAGE-SOURCE: expected 7 package source files, observed $($packageFiles.Count)"
}
$packageText = ($packageFiles | ForEach-Object {
    [System.IO.File]::ReadAllText($_.FullName)
}) -join "`n"
$packageReaderText = [System.IO.File]::ReadAllText((Join-Path $packageSource 'reader.rs'))

$forbiddenAuthorityPattern = '(?is)https?://|\bstd\s*::\s*process\b|\bCommand\s*::\s*new\b|\b(?:TcpStream|TcpListener|UdpSocket)\b|\b(?:reqwest|ureq|webbrowser|headless_chrome|zip|tar|sevenz|libarchive|slint|rusqlite)\s*::|\bplugin\b|powershell(?:\.exe)?|cmd(?:\.exe)?|bash(?:\.exe)?|\bsh\s+-c\b|\bAuthorization\s*:\s*Bearer\b'
if ($packageText -match $forbiddenAuthorityPattern) {
    throw 'TM-BACKUP-FORBIDDEN-AUTHORITY: package codec gained process/network/shell/generic-extraction/plugin/UI/SQL authority'
}
if (@([regex]::Matches($packageReaderText, 'if\s+settings\.source_schema_version\(\)\s*!=\s*manifest\.settings_schema_version\s*\{\s*return\s+Err\(StateError::integrity\(\)\);\s*\}')).Count -ne 1) {
    throw 'TM-BACKUP-SETTINGS-VERSION-BINDING: manifest and settings entry source versions must match exactly'
}
$expectedPublicPackageFunctions = [ordered]@{
    compression = 1
    copy_verified_stage_to_durable = 1
    created_at_utc_ms = 2
    database_len = 1
    database_schema_version = 1
    database_sha256 = 1
    decrypt = 1
    encrypt = 1
    existing = 1
    inspect = 1
    level = 1
    metadata = 1
    new = 2
    output_len = 1
    output_sha256 = 1
    package_len = 1
    package_sha256 = 1
    purpose = 1
    read = 2
    read_for_recovery = 1
    receipt = 2
    settings = 2
    verify_backup_stage = 1
    write = 2
    write_to_backup_stage = 1
    write_verified_candidate_to_backup_stage = 1
}
$publicPackageFunctions = @([regex]::Matches(
    $packageText,
    '(?m)^\s*pub\s+(?:(?:const|async|unsafe)\s+)*fn\s+(?<name>[A-Za-z_][A-Za-z0-9_]*)'
))
$publicPackageFunctionCounts = @{}
foreach ($function in $publicPackageFunctions) {
    $name = $function.Groups['name'].Value
    if ($publicPackageFunctionCounts.ContainsKey($name)) {
        $publicPackageFunctionCounts[$name] += 1
    } else {
        $publicPackageFunctionCounts[$name] = 1
    }
}
$unexpectedPublicPackageFunctions = @(
    $publicPackageFunctionCounts.Keys |
        Where-Object { -not $expectedPublicPackageFunctions.Contains($_) }
)
$driftedPublicPackageFunctions = @(
    $expectedPublicPackageFunctions.Keys |
        Where-Object {
            -not $publicPackageFunctionCounts.ContainsKey($_) -or
            $publicPackageFunctionCounts[$_] -ne $expectedPublicPackageFunctions[$_]
        }
)
$publicPackageFunctionDeclarations = @([regex]::Matches(
    $packageText,
    '(?ms)^\s*pub\s+(?:(?:async|unsafe)\s+)*fn\s+[A-Za-z_][A-Za-z0-9_]*\s*(?:<[^{}\r\n]*>)?\s*\((?<parameters>.*?)\)\s*(?:->|where|\{)'
))
$rawPublicPackageFunctions = @(
    $publicPackageFunctionDeclarations |
        Where-Object {
            $_.Value -cmatch '(?<![A-Za-z0-9_])(?:(?:dyn|impl)\s+)?(?:Read|Write)(?![A-Za-z0-9_])'
        }
)
if ($publicPackageFunctions.Count -ne 32 -or
    $unexpectedPublicPackageFunctions.Count -ne 0 -or
    $driftedPublicPackageFunctions.Count -ne 0 -or
    $rawPublicPackageFunctions.Count -ne 0) {
    throw 'TM-BACKUP-PACKAGE-CAPABILITY: public raw package writer or extractor authority is forbidden'
}

$coverageContracts = @(
    @{ File = 'fault'; Anchor = 'fn every_package_prefix_and_one_bit_mutation_fails_closed()' },
    @{ File = 'fault'; Anchor = 'fn preexisting_wal_and_shm_drift_fails_before_any_archive_move()' },
    @{ File = 'fault'; Anchor = 'fn prepared_resume_completes_an_exact_partially_moved_sidecar_set()' },
    @{ File = 'fault'; Anchor = 'fn conflicting_resumed_sidecar_target_fails_before_any_active_move()' },
    @{ File = 'package'; Anchor = 'fn truncation_and_flips_at_every_structural_region_fail_closed()' },
    @{ File = 'package'; Anchor = 'fn duplicate_unknown_entries_codecs_and_trailing_data_fail_closed()' },
    @{ File = 'package'; Anchor = 'fn decompression_bomb_content_size_lie_never_writes_past_declared_bound()' },
    @{ File = 'package'; Anchor = 'fn zstd_frame_advertising_more_than_the_eight_mib_window_is_rejected()' },
    @{ File = 'package'; Anchor = 'fn package_wire_and_debug_surfaces_contain_no_private_archive_metadata()' },
    @{ File = 'package'; Anchor = 'fn synthetic_exported_archive_is_free_of_private_input_canaries()' },
    @{ File = 'encryption'; Anchor = 'fn export_pins_scrypt_16_and_import_rejects_more_before_derivation()' },
    @{ File = 'encryption'; Anchor = 'fn wrong_password_and_every_outer_integrity_failure_poison_database_stage()' },
    @{ File = 'backup'; Anchor = 'fn header_page_and_index_corruption_have_distinct_stable_categories()' },
    @{ File = 'backup'; Anchor = 'fn foreign_key_schema_count_generation_and_semantic_failures_are_independent()' },
    @{ File = 'backup'; Anchor = 'fn every_backup_error_is_path_and_sqlite_text_private()' },
    @{ File = 'platform'; Anchor = 'fn existing_main_and_sidecars_move_as_one_reversible_quarantine_set()' },
    @{ File = 'platform'; Anchor = 'fn recovery_preflights_actual_available_staging_capacity()' },
    @{ File = 'app'; Anchor = 'fn application_recovery_and_migration_matrix_remains_executable()' },
    @{ File = 'app'; Anchor = 'fn application_gate_is_bound_to_the_complete_state_recovery_matrix()' },
    @{ File = 'app'; Anchor = 'mod automatic_recovery_contract;' },
    @{ File = 'app'; Anchor = 'mod maintenance_contract;' },
    @{ File = 'app'; Anchor = 'mod recovery_journal_contract;' },
    @{ File = 'app'; Anchor = 'mod restore_contract;' }
)
foreach ($contract in $coverageContracts) {
    $text = [System.IO.File]::ReadAllText($coverageFiles[$contract.File])
    if ([regex]::Matches($text, [regex]::Escape($contract.Anchor)).Count -ne 1) {
        throw "TM-BACKUP-TEST-MATRIX: missing exact anchor $($contract.Anchor)"
    }
}

$upstreamText = [System.IO.File]::ReadAllText($upstreamManifest)
if ([regex]::Matches($upstreamText, '(?m)^license\s*=\s*"MIT"\s*$').Count -ne 2 -or
    [regex]::Matches($upstreamText, '(?m)^commit\s*=\s*"[0-9a-f]{40}"\s*$').Count -ne 2) {
    throw 'TM-BACKUP-UPSTREAM-LICENSE: both pinned references must retain exact MIT declarations and immutable commits'
}
foreach ($licenseFile in $licenseFiles) {
    $licenseText = [System.IO.File]::ReadAllText($licenseFile)
    if ($licenseText -notmatch '(?m)^MIT License\s*$' -or
        $licenseText -notmatch 'Permission is hereby granted, free of charge') {
        throw "TM-BACKUP-UPSTREAM-LICENSE: invalid notice $([System.IO.Path]::GetFileName($licenseFile))"
    }
}

$productionPrivacyFiles = @(
    Get-ChildItem -LiteralPath (Join-Path $root 'crates') -Recurse -File |
        Where-Object {
            $_.Extension -in @('.rs', '.slint') -and
            $_.FullName -notmatch '[\\/]tests[\\/]' -and
            $_.Name -notlike '*_tests.rs' -and
            $_.FullName -notmatch '[\\/]src[\\/]bin[\\/]'
        } |
        Sort-Object FullName
)
$privacyText = ($productionPrivacyFiles + ($privacySurfaceFiles | ForEach-Object {
    Get-Item -LiteralPath $_
}) | Sort-Object FullName -Unique | ForEach-Object {
    [System.IO.File]::ReadAllText($_.FullName)
}) -join "`n"
$privateCanaries = @(
    'C:\private\codex-home',
    '/home/private/tokenmaster',
    'Private@Example.com',
    'PRIVATE_SESSION_NAME.jsonl',
    'PIPELINE_PRIVATE_SENTINEL_91A7',
    'Authorization: Bearer private',
    'prompt-private-canary',
    'response-private-canary',
    'reasoning-private-canary',
    'command-private-canary',
    'source-private-canary'
)
foreach ($canary in $privateCanaries) {
    if ($privacyText.IndexOf($canary, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "TM-BACKUP-PRIVATE-CANARY: production surface contains $canary"
    }
}

if ($SourceOnly) {
    [ordered]@{
        result = 'pass'
        scope = 'source-only'
        package_source_file_count = $packageFiles.Count
        coverage_anchor_count = $coverageContracts.Count
        external_reference_license_count = $licenseFiles.Count
        forbidden_authority_count = 0
        private_canary_count = 0
    } | ConvertTo-Json -Compress
    return
}

$lockFile = Join-Path $root 'Cargo.lock'
if (-not (Test-Path -LiteralPath $lockFile)) {
    throw 'TM-BACKUP-MISSING-BOUNDARY: Cargo.lock'
}
$cargo = (Get-Command cargo.exe -CommandType Application -ErrorAction Stop).Source
$metadataJson = & $cargo +1.97.0 metadata --locked --format-version 1 --manifest-path $rootManifest
if ($LASTEXITCODE -ne 0) {
    throw 'TM-BACKUP-METADATA: cargo metadata failed'
}
$metadata = $metadataJson | ConvertFrom-Json -Depth 100
$statePackages = @($metadata.packages | Where-Object { $_.name -eq 'tokenmaster-state' })
if ($statePackages.Count -ne 1) {
    throw 'TM-BACKUP-METADATA: tokenmaster-state must resolve exactly once'
}
$nodesById = @{}
foreach ($node in $metadata.resolve.nodes) {
    $nodesById[[string]$node.id] = $node
}
$packagesById = @{}
foreach ($package in $metadata.packages) {
    $packagesById[[string]$package.id] = $package
}
$pending = [System.Collections.Generic.Queue[string]]::new()
$pending.Enqueue([string]$statePackages[0].id)
$closure = [System.Collections.Generic.HashSet[string]]::new()
while ($pending.Count -gt 0) {
    $id = $pending.Dequeue()
    if (-not $closure.Add($id)) {
        continue
    }
    foreach ($dependency in $nodesById[$id].deps) {
        if (@($dependency.dep_kinds | Where-Object { $null -eq $_.kind }).Count -gt 0) {
            $pending.Enqueue([string]$dependency.pkg)
        }
    }
}
$closurePackages = @($closure | ForEach-Object { $packagesById[$_] })
foreach ($package in $closurePackages) {
    $license = [string]$package.license
    if ([string]::IsNullOrWhiteSpace($license)) {
        throw "TM-BACKUP-DEPENDENCY-LICENSE: missing license for $($package.name)"
    }
    if ($license -match '(?i)\b(?:AGPL|GPL|LGPL)' -and
        $license -notmatch '(?i)\b(?:MIT|Apache|BSD|Unlicense|Zlib|Unicode)') {
        throw "TM-BACKUP-DEPENDENCY-LICENSE: non-permissive-only license for $($package.name): $license"
    }
}

function Get-PolicySha256 {
    param([Parameter(Mandatory = $true)][string[]]$Records)

    $normalized = [string]::Join("`n", $Records)
    $algorithm = [System.Security.Cryptography.SHA256]::Create()
    try {
        return [System.Convert]::ToHexString(
            $algorithm.ComputeHash([System.Text.Encoding]::UTF8.GetBytes($normalized))
        ).ToLowerInvariant()
    }
    finally {
        $algorithm.Dispose()
    }
}

$dependencyPolicyRecords = @(
    $closure | ForEach-Object {
        $package = $packagesById[$_]
        '{0}|{1}|{2}' -f $package.name, $package.version, $package.license
    } | Sort-Object
)
$dependencyPolicySha256 = Get-PolicySha256 -Records $dependencyPolicyRecords
if ($dependencyPolicySha256 -ne $expectedDependencyPolicySha256) {
    throw "TM-BACKUP-DEPENDENCY-POLICY: exact name/version/license closure drifted: $dependencyPolicySha256"
}

$featurePolicyRecords = @(
    $closure | ForEach-Object {
        $package = $packagesById[$_]
        $node = $nodesById[$_]
        $features = @($node.features | Sort-Object) -join ','
        '{0}|{1}|{2}' -f $package.name, $package.version, $features
    } | Sort-Object
)
$featurePolicySha256 = Get-PolicySha256 -Records $featurePolicyRecords
if ($featurePolicySha256 -ne $expectedFeaturePolicySha256) {
    throw "TM-BACKUP-FEATURE-POLICY: exact resolved feature closure drifted: $featurePolicySha256"
}
foreach ($pin in @(
    @{ Name = 'age'; Version = '0.12.1' },
    @{ Name = 'zstd'; Version = '0.13.3' }
)) {
    $resolved = @($closurePackages | Where-Object {
        $_.name -eq $pin.Name -and [string]$_.version -eq $pin.Version
    })
    if ($resolved.Count -ne 1) {
        throw "TM-BACKUP-DEPENDENCY-PIN: $($pin.Name) $($pin.Version) must resolve exactly once"
    }
}

$featureTree = (& $cargo +1.97.0 tree -p tokenmaster-state -e features --manifest-path $rootManifest) -join "`n"
if ($LASTEXITCODE -ne 0) {
    throw 'TM-BACKUP-DEPENDENCY-FEATURES: cargo feature tree failed'
}
if ($featureTree -match '(?i)zstd(?:-safe|-sys)? feature "(?:zstdmt|training|legacy|experimental)"' -or
    $featureTree -match '(?i)\bage feature "(?:armor|async|cli-common|plugin|ssh|unstable|web-sys)"') {
    throw 'TM-BACKUP-DEPENDENCY-FEATURES: forbidden package feature entered the state closure'
}

& $cargo +1.97.0 build --release --locked --manifest-path $rootManifest -p tokenmaster-app
if ($LASTEXITCODE -ne 0) {
    throw 'TM-BACKUP-BUILD: release application build failed'
}
$targetDirectory = [System.IO.Path]::GetFullPath([string]$metadata.target_directory)
$artifacts = @(
    Get-ChildItem -LiteralPath $targetDirectory -Recurse -File -Filter 'TokenMaster.exe' |
        Where-Object { $_.FullName -match '[\\/]release[\\/]TokenMaster\.exe$' }
)
if ($artifacts.Count -ne 1) {
    throw 'TM-BACKUP-ARTIFACT: release TokenMaster executable was not found exactly once'
}
$artifactText = [System.Text.Encoding]::ASCII.GetString(
    [System.IO.File]::ReadAllBytes($artifacts[0].FullName)
)
foreach ($canary in $privateCanaries) {
    if ($artifactText.IndexOf($canary, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "TM-BACKUP-PRIVATE-CANARY: release binary contains $canary"
    }
}

[ordered]@{
    result = 'pass'
    scope = 'workspace'
    package_source_file_count = $packageFiles.Count
    coverage_anchor_count = $coverageContracts.Count
    external_reference_license_count = $licenseFiles.Count
    dependency_closure_count = $closurePackages.Count
    dependency_license_count = $closurePackages.Count
    dependency_policy_sha256 = $dependencyPolicySha256
    feature_policy_sha256 = $featurePolicySha256
    forbidden_authority_count = 0
    private_canary_count = 0
    production_privacy_source_count = $productionPrivacyFiles.Count
    release_artifact_count = $artifacts.Count
    synthetic_archive_privacy_contract_count = 1
} | ConvertTo-Json -Compress
