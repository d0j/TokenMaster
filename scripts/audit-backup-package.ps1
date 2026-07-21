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

function Get-RustCodeMask {
    param([Parameter(Mandatory = $true)][string]$Text)

    $characters = $Text.ToCharArray()
    $length = $characters.Length
    $index = 0
    while ($index -lt $length) {
        $next = if ($index + 1 -lt $length) { $characters[$index + 1] } else { [char]0 }
        if ($characters[$index] -eq '/' -and $next -eq '/') {
            while ($index -lt $length -and $characters[$index] -ne "`n") {
                $characters[$index] = ' '
                $index += 1
            }
            continue
        }
        if ($characters[$index] -eq '/' -and $next -eq '*') {
            $depth = 1
            $characters[$index] = ' '
            $characters[$index + 1] = ' '
            $index += 2
            while ($index -lt $length -and $depth -gt 0) {
                $next = if ($index + 1 -lt $length) { $characters[$index + 1] } else { [char]0 }
                if ($characters[$index] -eq '/' -and $next -eq '*') {
                    $depth += 1
                    $characters[$index] = ' '
                    $characters[$index + 1] = ' '
                    $index += 2
                    continue
                }
                if ($characters[$index] -eq '*' -and $next -eq '/') {
                    $depth -= 1
                    $characters[$index] = ' '
                    $characters[$index + 1] = ' '
                    $index += 2
                    continue
                }
                if ($characters[$index] -ne "`r" -and $characters[$index] -ne "`n") {
                    $characters[$index] = ' '
                }
                $index += 1
            }
            continue
        }

        $rawPrefixStart = -1
        $rawMarker = -1
        $previousIsIdentifier = $index -gt 0 -and $characters[$index - 1] -match '[A-Za-z0-9_]'
        if (-not $previousIsIdentifier -and $characters[$index] -eq 'r') {
            $rawPrefixStart = $index
            $rawMarker = $index
        } elseif (-not $previousIsIdentifier -and $characters[$index] -eq 'b' -and
            $index + 1 -lt $length -and $characters[$index + 1] -eq 'r') {
            $rawPrefixStart = $index
            $rawMarker = $index + 1
        }
        if ($rawMarker -ge 0) {
            $quote = $rawMarker + 1
            while ($quote -lt $length -and $characters[$quote] -eq '#') {
                $quote += 1
            }
            if ($quote -lt $length -and $characters[$quote] -eq '"') {
                $hashCount = $quote - $rawMarker - 1
                $cursor = $quote + 1
                $end = -1
                while ($cursor -lt $length) {
                    if ($characters[$cursor] -eq '"') {
                        $matchesTerminator = $true
                        for ($hashIndex = 0; $hashIndex -lt $hashCount; $hashIndex += 1) {
                            if ($cursor + 1 + $hashIndex -ge $length -or
                                $characters[$cursor + 1 + $hashIndex] -ne '#') {
                                $matchesTerminator = $false
                                break
                            }
                        }
                        if ($matchesTerminator) {
                            $end = $cursor + $hashCount
                            break
                        }
                    }
                    $cursor += 1
                }
                if ($end -lt 0) {
                    throw 'TM-BACKUP-PACKAGE-CAPABILITY: unterminated Rust raw string'
                }
                for ($cursor = $rawPrefixStart; $cursor -le $end; $cursor += 1) {
                    if ($characters[$cursor] -ne "`r" -and $characters[$cursor] -ne "`n") {
                        $characters[$cursor] = ' '
                    }
                }
                $index = $end + 1
                continue
            }
        }

        if ($characters[$index] -eq '"') {
            $characters[$index] = ' '
            $index += 1
            $escaped = $false
            $closed = $false
            while ($index -lt $length) {
                $character = $characters[$index]
                if ($character -ne "`r" -and $character -ne "`n") {
                    $characters[$index] = ' '
                }
                if (-not $escaped -and $character -eq '"') {
                    $closed = $true
                    $index += 1
                    break
                }
                if (-not $escaped -and $character -eq '\') {
                    $escaped = $true
                } else {
                    $escaped = $false
                }
                $index += 1
            }
            if (-not $closed) {
                throw 'TM-BACKUP-PACKAGE-CAPABILITY: unterminated Rust string'
            }
            continue
        }
        $index += 1
    }
    return -join $characters
}

function Get-PublicRustFunctionSignatures {
    param(
        [Parameter(Mandatory = $true)][string]$Text,
        [Parameter(Mandatory = $true)][string]$FileName
    )

    $mask = Get-RustCodeMask -Text $Text
    $declarationStarts = @([regex]::Matches(
        $mask,
        '\bpub[ \t\r\n]+(?:(?:[A-Za-z_][A-Za-z0-9_]*)[ \t\r\n]+)*fn[ \t\r\n]+(?<name>[A-Za-z_][A-Za-z0-9_]*)'
    ))
    $result = @()
    foreach ($declarationStart in $declarationStarts) {
        $parenthesisDepth = 0
        $bracketDepth = 0
        $angleDepth = 0
        $nestedBraceDepth = 0
        $sawParameters = $false
        $signatureEnd = -1
        for ($index = $declarationStart.Index; $index -lt $mask.Length; $index += 1) {
            $character = $mask[$index]
            switch ($character) {
                '(' {
                    $parenthesisDepth += 1
                    $sawParameters = $true
                }
                ')' {
                    if ($parenthesisDepth -le 0) {
                        throw 'TM-BACKUP-PACKAGE-CAPABILITY: unbalanced public function parentheses'
                    }
                    $parenthesisDepth -= 1
                }
                '[' { $bracketDepth += 1 }
                ']' {
                    if ($bracketDepth -le 0) {
                        throw 'TM-BACKUP-PACKAGE-CAPABILITY: unbalanced public function brackets'
                    }
                    $bracketDepth -= 1
                }
                '<' { $angleDepth += 1 }
                '>' {
                    $previous = if ($index -gt 0) { $mask[$index - 1] } else { [char]0 }
                    if ($angleDepth -gt 0 -and $previous -ne '-') {
                        $angleDepth -= 1
                    }
                }
                '{' {
                    if ($sawParameters -and $parenthesisDepth -eq 0 -and $bracketDepth -eq 0 -and
                        $angleDepth -eq 0 -and $nestedBraceDepth -eq 0) {
                        $signatureEnd = $index
                        break
                    }
                    $nestedBraceDepth += 1
                }
                '}' {
                    if ($nestedBraceDepth -gt 0) {
                        $nestedBraceDepth -= 1
                    }
                }
                ';' {
                    if ($sawParameters -and $parenthesisDepth -eq 0 -and $bracketDepth -eq 0 -and
                        $angleDepth -eq 0 -and $nestedBraceDepth -eq 0) {
                        $signatureEnd = $index
                        break
                    }
                }
            }
            if ($signatureEnd -ge 0) {
                break
            }
        }
        if (-not $sawParameters -or $signatureEnd -lt 0) {
            throw 'TM-BACKUP-PACKAGE-CAPABILITY: public Rust function declaration did not parse'
        }
        $normalized = $mask.Substring(
            $declarationStart.Index,
            $signatureEnd - $declarationStart.Index
        ) -replace '\s+', ''
        $result += [pscustomobject]@{
            FileName = $FileName
            Name = $declarationStart.Groups['name'].Value
            Signature = $normalized
        }
    }
    return $result
}

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
$expectedPublicPackageSurface = @(
    'encryption.rs|pubconstfnoutput_len(self)->u64'
    'encryption.rs|pubconstfnoutput_sha256(&self)->&[u8;32]'
    'encryption.rs|pubfndecrypt(source:&mutDurableFileReader,passphrase:BackupPassphrase,database_destination:&mutDurableStagedFile,)->Result<VerifiedBackupPackage,StateError>'
    'encryption.rs|pubfnencrypt(context:BackupEncryptionContext,source:&mutDurableFileReader,verified:&VerifiedBackupPackage,passphrase:BackupPassphrase,destination:&mutDurableStagedFile,)->Result<ProtectedPackageReceipt,StateError>'
    'encryption.rs|pubfnexisting(input:&mutString)->Result<Self,StateError>'
    'encryption.rs|pubfnnew(input:&mutString,confirmation:&mutString)->Result<Self,StateError>'
    'mod.rs|pubconstfncompression(&self)->BackupCompression'
    'mod.rs|pubconstfncreated_at_utc_ms(&self)->i64'
    'mod.rs|pubconstfncreated_at_utc_ms(self)->i64'
    'mod.rs|pubconstfndatabase_len(&self)->u64'
    'mod.rs|pubconstfndatabase_schema_version(&self)->u16'
    'mod.rs|pubconstfndatabase_sha256(&self)->&[u8;32]'
    'mod.rs|pubconstfnlevel(self)->i32'
    'mod.rs|pubconstfnmetadata(&self)->BackupMetadata'
    'mod.rs|pubconstfnpackage_len(self)->u64'
    'mod.rs|pubconstfnpackage_sha256(&self)->&[u8;32]'
    'mod.rs|pubconstfnpurpose(self)->BackupPurpose'
    'mod.rs|pubconstfnreceipt(&self)->PackageReceipt'
    'mod.rs|pubconstfnreceipt(&self)->PackageReceipt'
    'mod.rs|pubconstfnsettings(&self)->&PortableSettingsCandidate'
    'mod.rs|pubconstfnsettings(&self)->&PortableSettingsCandidate'
    'mod.rs|pubfnnew(created_at_utc_ms:i64,purpose:BackupPurpose)->Result<Self,crate::StateError>'
    'reader.rs|pubfninspect(source:&mutDurableFileReader)->Result<VerifiedBackupPackage,StateError>'
    'reader.rs|pubfnread(source:&mutDurableFileReader)->Result<VerifiedConfigPackage,StateError>'
    'reader.rs|pubfnread(source:&mutDurableFileReader,database_sink:&mutDurableStagedFile,)->Result<VerifiedBackupPackage,StateError>'
    'reader.rs|pubfnread_for_recovery(source:&mutDurableFileReader,database_sink:&mutRecoveryStagedArchive,)->Result<VerifiedBackupPackage,StateError>'
    'reader.rs|pubfnverify_backup_stage(source:&BackupStagedFile,)->Result<VerifiedBackupPackage,StateError>'
    'writer.rs|pubfncopy_verified_stage_to_durable(source:&BackupStagedFile,verified:&VerifiedBackupPackage,destination:&mutDurableStagedFile,)->Result<PackageReceipt,StateError>'
    'writer.rs|pubfnwrite(settings:&PortableSettingsCandidate,created_at_utc_ms:i64,destination:&mutDurableStagedFile,)->Result<PackageReceipt,StateError>'
    'writer.rs|pubfnwrite(settings:&PortableSettingsCandidate,database:&mutDurableFileReader,database_len:u64,database_sha256:[u8;32],database_schema_version:u16,compression:BackupCompression,metadata:BackupMetadata,destination:&mutDurableStagedFile,)->Result<PackageReceipt,StateError>'
    'writer.rs|pubfnwrite_to_backup_stage(settings:&PortableSettingsCandidate,database:&mutDurableFileReader,database_len:u64,database_sha256:[u8;32],database_schema_version:u16,compression:BackupCompression,metadata:BackupMetadata,destination:&mutBackupStagedFile,)->Result<PackageReceipt,StateError>'
    "writer.rs|pubfnwrite_verified_candidate_to_backup_stage(settings:&PortableSettingsCandidate,mutdatabase:VerifiedBackupCandidateReader<'_>,compression:BackupCompression,metadata:BackupMetadata,destination:&mutBackupStagedFile,)->Result<PackageReceipt,StateError>"
)
$publicPackageFunctionDeclarations = @(
    $packageFiles | ForEach-Object {
        Get-PublicRustFunctionSignatures `
            -Text ([System.IO.File]::ReadAllText($_.FullName)) `
            -FileName $_.Name
    }
)
$actualPublicPackageSurface = @(
    $publicPackageFunctionDeclarations |
        ForEach-Object { "$($_.FileName)|$($_.Signature)" } |
        Sort-Object -CaseSensitive
)
$canonicalExpectedPublicPackageSurface = @(
    $expectedPublicPackageSurface | Sort-Object -CaseSensitive
) -join "`n"
$canonicalActualPublicPackageSurface = $actualPublicPackageSurface -join "`n"
$rawPublicPackageFunctions = @(
    $publicPackageFunctionDeclarations |
        Where-Object {
            $_.Signature -cmatch '(?<![A-Za-z0-9_])(?:(?:dyn|impl))?(?:Read|Write)(?![A-Za-z0-9_])'
        }
)
if ($publicPackageFunctionDeclarations.Count -ne 32 -or
    $canonicalActualPublicPackageSurface -cne $canonicalExpectedPublicPackageSurface -or
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
