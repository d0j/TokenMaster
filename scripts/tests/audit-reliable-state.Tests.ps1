Describe "TokenMaster reliable-state authority audit" {
    BeforeAll {
        $ScriptsRoot = Split-Path -Parent $PSScriptRoot
        $RepositoryRoot = (Resolve-Path (Join-Path $ScriptsRoot "..")).Path
        $Audit = Join-Path $ScriptsRoot "audit-reliable-state.ps1"

        function New-StateAuditFixture {
            param([Parameter(Mandatory = $true)][string]$Name)

            $fixture = Join-Path $TestDrive $Name
            New-Item -ItemType Directory -Path $fixture -Force | Out-Null
            Copy-Item -LiteralPath (Join-Path $RepositoryRoot "Cargo.toml") -Destination $fixture
            $crateParent = Join-Path $fixture "crates"
            New-Item -ItemType Directory -Path $crateParent -Force | Out-Null
            Copy-Item -LiteralPath (Join-Path $RepositoryRoot "crates\state") `
                -Destination $crateParent -Recurse
            return $fixture
        }

        function New-WorkspaceMembershipBypassFixture {
            param(
                [Parameter(Mandatory = $true)][string]$Name,
                [switch]$IncludeStateMember,
                [string]$PlatformDependency = ''
            )

            $fixture = Join-Path $TestDrive $Name
            $stateRoot = Join-Path $fixture "crates\state"
            $platformRoot = Join-Path $fixture "crates\platform"
            $storeRoot = Join-Path $fixture "crates\store"
            $hostRoot = Join-Path $fixture "crates\host"
            New-Item -ItemType Directory -Path $stateRoot, $platformRoot, $storeRoot, $hostRoot -Force |
                Out-Null
            Copy-Item -LiteralPath (Join-Path $RepositoryRoot "crates\state\src") `
                -Destination $stateRoot -Recurse

            $rootManifest = if ($IncludeStateMember) {
                @'
[workspace]
resolver = "3"
members = ["crates/state", "crates/platform", "crates/store"]

[workspace.dependencies]
age = { version = "=0.12.1", default-features = false }
zstd = { version = "=0.13.3", default-features = false }
'@
            } else {
                @'
[workspace]
resolver = "3"
members = ["crates/host"]
exclude = ["crates/state", "crates/platform"]

[workspace.dependencies]
age = { version = "=0.12.1", default-features = false }
zstd = { version = "=0.13.3", default-features = false }
'@
            }
            $rootManifest | Set-Content -LiteralPath (Join-Path $fixture "Cargo.toml")

            @'
[package]
name = "tokenmaster-state"
version = "0.1.0"
edition = "2024"
license = "MIT"
rust-version = "1.97"

[dependencies]
age.workspace = true
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.11"
thiserror = "2"
tokenmaster-platform = { path = "../platform" }
tokenmaster-store = { path = "../store" }
zstd.workspace = true
'@ | Set-Content -LiteralPath (Join-Path $stateRoot "Cargo.toml")
            if (-not $IncludeStateMember) {
                $excludedStateManifest = Join-Path $stateRoot "Cargo.toml"
                $excludedStateText = [System.IO.File]::ReadAllText($excludedStateManifest)
                [System.IO.File]::WriteAllText(
                    $excludedStateManifest,
                    $excludedStateText.
                        Replace(
                            'age.workspace = true',
                            'age = { version = "=0.12.1", default-features = false }'
                        ).
                        Replace(
                            'zstd.workspace = true',
                            'zstd = { version = "=0.13.3", default-features = false }'
                        )
                )
            }

            New-Item -ItemType Directory -Path (Join-Path $platformRoot "src") -Force |
                Out-Null
            @'
[package]
name = "tokenmaster-platform"
version = "0.1.0"
edition = "2024"
'@ + $(if ($PlatformDependency) { "`n[dependencies]`n$PlatformDependency`n" } else { '' }) |
                Set-Content -LiteralPath (Join-Path $platformRoot "Cargo.toml")
            "#![forbid(unsafe_code)]" |
                Set-Content -LiteralPath (Join-Path $platformRoot "src\lib.rs")

            New-Item -ItemType Directory -Path (Join-Path $storeRoot "src") -Force |
                Out-Null
            @'
[package]
name = "tokenmaster-store"
version = "0.1.0"
edition = "2024"
'@ | Set-Content -LiteralPath (Join-Path $storeRoot "Cargo.toml")
            "#![forbid(unsafe_code)]" |
                Set-Content -LiteralPath (Join-Path $storeRoot "src\lib.rs")

            New-Item -ItemType Directory -Path (Join-Path $hostRoot "src") -Force |
                Out-Null
            @'
[package]
name = "state-audit-host"
version = "0.1.0"
edition = "2024"

[dependencies]
tokenmaster-state = { path = "../state" }
'@ | Set-Content -LiteralPath (Join-Path $hostRoot "Cargo.toml")
            "#![forbid(unsafe_code)]" |
                Set-Content -LiteralPath (Join-Path $hostRoot "src\lib.rs")

            $cargo = (Get-Command cargo.exe -CommandType Application -ErrorAction Stop).Source
            & $cargo +1.97.0 generate-lockfile --offline `
                --manifest-path (Join-Path $fixture "Cargo.toml")
            if ($LASTEXITCODE -ne 0) {
                throw "failed to create membership-bypass fixture lockfile"
            }
            return $fixture
        }
    }

    It "accepts the current source boundary" {
        $result = & $Audit -RepositoryRoot $RepositoryRoot -SourceOnly | ConvertFrom-Json

        $result.result | Should -Be "pass"
        $result.package | Should -Be "tokenmaster-state"
        $result.binary_target_count | Should -Be 0
        $result.direct_production_dependency_count | Should -Be 8
        $result.approved_std_io_import_count | Should -Be 5
        $result.approved_maintenance_std_import_count | Should -Be 4
        $result.approved_store_candidate_import_count | Should -Be 1
        $result.approved_maintenance_store_control_import_count | Should -Be 1
        $result.approved_platform_import_count | Should -Be 6
        $result.forbidden_authority_count | Should -Be 0
    }

    It "rejects zstd version or feature drift" {
        $fixture = New-StateAuditFixture -Name "zstd-drift"
        $manifest = Join-Path $fixture "Cargo.toml"
        $text = [System.IO.File]::ReadAllText($manifest)
        [System.IO.File]::WriteAllText(
            $manifest,
            $text.Replace(
                'zstd = { version = "=0.13.3", default-features = false }',
                'zstd = { version = "0.13", features = ["zstdmt"] }'
            )
        )

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-ZSTD-PIN*"
    }

    It "rejects age version or feature drift" {
        $fixture = New-StateAuditFixture -Name "age-drift"
        $manifest = Join-Path $fixture "Cargo.toml"
        $text = [System.IO.File]::ReadAllText($manifest)
        [System.IO.File]::WriteAllText(
            $manifest,
            $text.Replace(
                'age = { version = "=0.12.1", default-features = false }',
                'age = { version = "0.12", features = ["ssh"] }'
            )
        )

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-AGE-PIN*"
    }

    It "rejects store interop dependency source drift" {
        $fixture = New-StateAuditFixture -Name "store-source-drift"
        $manifest = Join-Path $fixture "crates\state\Cargo.toml"
        $text = [System.IO.File]::ReadAllText($manifest)
        [System.IO.File]::WriteAllText(
            $manifest,
            $text.Replace(
                'tokenmaster-store = { path = "../store" }',
                'tokenmaster-store = "0.1"'
            )
        )

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-STORE-PIN*"
    }

    It "rejects a state binary target" {
        $fixture = New-StateAuditFixture -Name "binary-target"
        Add-Content -LiteralPath (Join-Path $fixture "crates\state\Cargo.toml") `
            -Value "`n[[bin]]`nname = `"state-helper`"`npath = `"src/lib.rs`""

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-BINARY-TARGET*"
    }

    It "rejects forbidden dependency <Name>" -TestCases @(
        @{ Name = "slint"; Dependency = "slint.workspace = true" }
        @{ Name = "runtime"; Dependency = "tokenmaster-runtime = { path = `"../runtime`" }" }
        @{ Name = "archive"; Dependency = "zip = `"1`"" }
        @{ Name = "async"; Dependency = "tokio = `"1`"" }
        @{ Name = "network"; Dependency = "reqwest = `"1`"" }
    ) {
        $fixture = New-StateAuditFixture -Name "dependency-$Name"
        Add-Content -LiteralPath (Join-Path $fixture "crates\state\Cargo.toml") `
            -Value $Dependency

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-DEPENDENCIES*"
    }

    It "rejects forbidden source authority <Name>" -TestCases @(
        @{ Name = "process"; Source = 'pub fn forbidden() { let _ = std::process::Command::new("cmd"); }' }
        @{ Name = "socket"; Source = 'pub fn forbidden() { let _ = std::net::TcpStream::connect("127.0.0.1:1"); }' }
        @{ Name = "network"; Source = 'pub const FORBIDDEN: &str = "https://example.invalid";' }
        @{ Name = "sql"; Source = 'pub const FORBIDDEN: &str = "SELECT * FROM state";' }
        @{ Name = "slint"; Source = 'pub fn forbidden(_: slint::Weak<slint::Window>) {}' }
    ) {
        $fixture = New-StateAuditFixture -Name "source-$Name"
        Add-Content -LiteralPath (Join-Path $fixture "crates\state\src\lib.rs") `
            -Value $Source

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-FORBIDDEN-AUTHORITY*"
    }

    It "rejects a public arbitrary-path constructor" {
        $fixture = New-StateAuditFixture -Name "public-path"
        Add-Content -LiteralPath (Join-Path $fixture "crates\state\src\lib.rs") `
            -Value 'pub fn from_path(_: &std::path::Path) {}'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-ARBITRARY-PATH*"
    }

    It "rejects a generic public arbitrary-path constructor" {
        $fixture = New-StateAuditFixture -Name "generic-public-path"
        Add-Content -LiteralPath (Join-Path $fixture "crates\state\src\lib.rs") `
            -Value 'pub fn open_path<P: AsRef<std::path::Path>>(_: P) {}'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-ARBITRARY-PATH*"
    }

    It "rejects public generic stream authority" {
        $fixture = New-StateAuditFixture -Name "public-stream"
        $source = Join-Path $fixture "crates\state\src\lib.rs"
        [System.IO.File]::AppendAllText(
            $source,
            "`npub fn leaked_stream<R: Read>(_source: R) {}`n"
        )

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-STREAM-AUTHORITY*"
    }

    It "does not count a commented workspace member" {
        $fixture = New-StateAuditFixture -Name "commented-workspace-member"
        $rootManifest = Join-Path $fixture "Cargo.toml"
        $text = [System.IO.File]::ReadAllText($rootManifest)
        [System.IO.File]::WriteAllText(
            $rootManifest,
            $text.Replace('  "crates/state",', '  # "crates/state",')
        )

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-WORKSPACE*"
    }

    It "rejects state reached only as an excluded path dependency" {
        $fixture = New-WorkspaceMembershipBypassFixture -Name "path-dependency-member"

        { & $Audit -RepositoryRoot $fixture } |
            Should -Throw "*TM-STATE-WORKSPACE*"
    }

    It "rejects forbidden authority entering through an allowed direct dependency" {
        $fixture = New-WorkspaceMembershipBypassFixture `
            -Name "transitive-authority" `
            -IncludeStateMember `
            -PlatformDependency 'tokio = { version = "1", features = ["rt"] }'

        { & $Audit -RepositoryRoot $fixture } |
            Should -Throw "*TM-STATE-TRANSITIVE-AUTHORITY*"
    }

    It "rejects source-level authority bypass <Name>" -TestCases @(
        @{ Name = "string-filesystem"; Source = 'pub fn load(input: &str) { let _ = std::fs::read(input); }' }
        @{ Name = "aliased-path"; Source = 'pub type P = std::path::Path; pub fn open(input: &P) { let _ = input; }' }
        @{ Name = "trait-path"; Source = 'pub trait Open { fn open<P: AsRef<std::path::Path>>(input: P); }' }
        @{ Name = "filesystem-reexport"; Source = 'pub use std::fs::read as load_state;' }
        @{ Name = "grouped-filesystem-import"; Source = 'use std::{fs as raw_fs}; pub fn load(input: &str) { let _ = raw_fs::read(input); }' }
        @{ Name = "external-include"; Source = 'pub const EMBEDDED: &[u8] = include_bytes!("outside.bin");' }
        @{ Name = "external-module"; Source = '#[path = "outside.rs"] mod outside;' }
        @{ Name = "standard-library-alias"; Source = 'use std as system; pub fn load(input: &str) { let _ = system::fs::read(input); }' }
        @{ Name = "standard-library-group-alias"; Source = 'use {std as system}; pub fn load(input: &str) { let _ = system::fs::read(input); }' }
        @{ Name = "platform-reexport"; Source = 'pub use tokenmaster_platform::ValidatedLocalDirectory;' }
        @{ Name = "platform-alias-reexport"; Source = 'use tokenmaster_platform::ExclusiveFileLease as Lease; pub use Lease as StateLease;' }
        @{ Name = "platform-private-authority"; Source = 'use tokenmaster_platform::ExclusiveFileLease;' }
        @{ Name = "declarative-macro"; Source = 'macro_rules! expose { ($root:ident) => { pub use $root::fs::read; } } expose!(std);' }
    ) {
        $fixture = New-StateAuditFixture -Name "authority-bypass-$Name"
        Add-Content -LiteralPath (Join-Path $fixture "crates\state\src\lib.rs") `
            -Value $Source

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-FORBIDDEN-AUTHORITY*"
    }

    It "rejects a direct store capability reexport" {
        $fixture = New-StateAuditFixture -Name "store-reexport"
        Add-Content -LiteralPath (Join-Path $fixture "crates\state\src\lib.rs") `
            -Value 'pub use tokenmaster_store::VerifiedBackupCandidateReader;'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-STORE-AUTHORITY*"
    }

    It "rejects std io authority through the approved alias" {
        $fixture = New-StateAuditFixture -Name "approved-io-alias-bypass"
        Add-Content -LiteralPath (Join-Path $fixture "crates\state\src\record.rs") `
            -Value 'fn forbidden_io() { let _ = io::stdout(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-APPROVED-IO*"
    }

    It "rejects caller-selected children through the approved platform alias" {
        $fixture = New-StateAuditFixture -Name "approved-platform-alias-bypass"
        Add-Content -LiteralPath (Join-Path $fixture "crates\state\src\record.rs") `
            -Value 'fn forbidden_child(directory: &ValidatedLocalDirectory, caller: &str) { let _ = DurableFileTarget::exact_child(directory, caller); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-EXACT-CHILD*"
    }

    It "rejects path disclosure through the approved settings directory capability" {
        $fixture = New-StateAuditFixture -Name "settings-directory-path-disclosure"
        Add-Content -LiteralPath (Join-Path $fixture "crates\state\src\settings\store.rs") `
            -Value 'pub fn reveal(directory: &ValidatedLocalDirectory) -> Option<&str> { directory . as_path ( ) . to_str ( ) }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-VALIDATED-DIRECTORY*"
    }

    It "rejects direct filesystem enumeration from the typed backup catalog" {
        $fixture = New-StateAuditFixture -Name "catalog-filesystem-bypass"
        Add-Content -LiteralPath (Join-Path $fixture "crates\state\src\catalog.rs") `
            -Value 'fn forbidden_scan() { let _ = std::fs::read_dir("."); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-FORBIDDEN-AUTHORITY*"
    }

    It "rejects raw platform backup tokens in public state methods" {
        $fixture = New-StateAuditFixture -Name "public-backup-token"
        Add-Content -LiteralPath (Join-Path $fixture "crates\state\src\catalog.rs") `
            -Value 'pub fn leak_backup_token(entry: BackupDirectoryEntry) { let _ = entry; }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-BACKUP-DIRECTORY-AUTHORITY*"
    }

    It "rejects a second public backup-stage writer escape" {
        $fixture = New-StateAuditFixture -Name "second-public-backup-stage-writer"
        Add-Content -LiteralPath (Join-Path $fixture "crates\state\src\package\writer.rs") `
            -Value 'pub fn write_to_backup_stage(destination: &mut BackupStagedFile) -> Result<PackageReceipt, StateError> { Err(StateError::unavailable()) }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-BACKUP-DIRECTORY-AUTHORITY*"
    }

    It "rejects a second public backup-stage verifier escape" {
        $fixture = New-StateAuditFixture -Name "second-public-backup-stage-verifier"
        Add-Content -LiteralPath (Join-Path $fixture "crates\state\src\package\reader.rs") `
            -Value 'pub fn verify_backup_stage(source: &BackupStagedFile) -> Result<VerifiedBackupPackage, StateError> { let _ = source; Err(StateError::unavailable()) }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-BACKUP-DIRECTORY-AUTHORITY*"
    }

    It "rejects a second verified-candidate package bridge" {
        $fixture = New-StateAuditFixture -Name "second-verified-candidate-writer"
        Add-Content -LiteralPath (Join-Path $fixture "crates\state\src\package\writer.rs") `
            -Value "pub fn leak_candidate(database: VerifiedBackupCandidateReader<'_>) { let _ = database; }"

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-STORE-AUTHORITY*"
    }

    It "rejects a second public store cancellation-control escape" {
        $fixture = New-StateAuditFixture -Name "second-store-control"
        Add-Content -LiteralPath (Join-Path $fixture "crates\state\src\maintenance\coordinator.rs") `
            -Value 'pub fn leak_control(control: BackupControl) { let _ = control; }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-STORE-AUTHORITY*"
    }

    It "rejects an alias escape of an approved store capability" {
        $fixture = New-StateAuditFixture -Name "store-capability-alias"
        Add-Content -LiteralPath (Join-Path $fixture "crates\state\src\maintenance\coordinator.rs") `
            -Value 'pub type LeakedBackupControl = BackupControl;'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-STORE-AUTHORITY*"
    }

    It "keeps backup staging publication sealed behind BackupDirectory" {
        $source = [System.IO.File]::ReadAllText(
            (Join-Path $RepositoryRoot "crates\platform\src\backup_directory.rs")
        )
        $block = [regex]::Match(
            $source,
            '(?s)impl\s+BackupStagedFile\s*\{(?<body>.*?)\n\}\s*\n\s*impl\s+fmt::Debug'
        )
        $block.Success | Should -BeTrue
        $methods = @(
            [regex]::Matches(
                $block.Groups['body'].Value,
                '\bpub\s+(?:const\s+)?fn\s+(?<name>[A-Za-z_][A-Za-z0-9_]*)'
            ) | ForEach-Object { $_.Groups['name'].Value } | Sort-Object
        )
        $methods | Should -Be @('discard', 'open_reader', 'seal', 'write_chunk', 'written_len')
        @([regex]::Matches($source, '\bpub\s+fn\s+publish\s*\(')).Count | Should -Be 1
    }

    It "rejects public generic record authority" {
        $fixture = New-StateAuditFixture -Name "public-record-authority"
        Add-Content -LiteralPath (Join-Path $fixture "crates\state\src\lib.rs") `
            -Value 'pub use record::RedundantRecordStore;'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-RECORD-VISIBILITY*"
    }
}
