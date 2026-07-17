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
            $hostRoot = Join-Path $fixture "crates\host"
            New-Item -ItemType Directory -Path $stateRoot, $platformRoot, $hostRoot -Force |
                Out-Null
            Copy-Item -LiteralPath (Join-Path $RepositoryRoot "crates\state\src") `
                -Destination $stateRoot -Recurse

            $rootManifest = if ($IncludeStateMember) {
                @'
[workspace]
resolver = "3"
members = ["crates/state", "crates/platform"]
'@
            } else {
                @'
[workspace]
resolver = "3"
members = ["crates/host"]
exclude = ["crates/state", "crates/platform"]
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
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sha2 = "0.11"
thiserror = "2"
tokenmaster-platform = { path = "../platform" }
'@ | Set-Content -LiteralPath (Join-Path $stateRoot "Cargo.toml")

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
        $result.direct_production_dependency_count | Should -Be 5
        $result.approved_std_io_import_count | Should -Be 1
        $result.approved_platform_import_count | Should -Be 1
        $result.forbidden_authority_count | Should -Be 0
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

    It "rejects public generic record authority" {
        $fixture = New-StateAuditFixture -Name "public-record-authority"
        Add-Content -LiteralPath (Join-Path $fixture "crates\state\src\lib.rs") `
            -Value 'pub use record::RedundantRecordStore;'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-STATE-RECORD-VISIBILITY*"
    }
}
