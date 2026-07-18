Describe "TokenMaster backup package audit" {
    BeforeAll {
        $ScriptsRoot = Split-Path -Parent $PSScriptRoot
        $RepositoryRoot = (Resolve-Path (Join-Path $ScriptsRoot "..")).Path
        $Audit = Join-Path $ScriptsRoot "audit-backup-package.ps1"

        function New-BackupAuditFixture {
            param([Parameter(Mandatory = $true)][string]$Name)

            $fixture = Join-Path $TestDrive $Name
            New-Item -ItemType Directory -Path $fixture -Force | Out-Null
            Copy-Item -LiteralPath (Join-Path $RepositoryRoot "Cargo.toml") -Destination $fixture
            foreach ($relative in @(
                "crates\state\Cargo.toml",
                "crates\state\src",
                "crates\state\tests",
                "crates\store\tests",
                "crates\platform\tests",
                "crates\app\src",
                "crates\app\tests",
                "crates\desktop\src\reliable_state.rs",
                "third_party"
            )) {
                $source = Join-Path $RepositoryRoot $relative
                $destination = Join-Path $fixture $relative
                New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force |
                    Out-Null
                Copy-Item -LiteralPath $source -Destination $destination -Recurse
            }
            return $fixture
        }
    }

    It "accepts the complete source and coverage boundary" {
        $result = & $Audit -RepositoryRoot $RepositoryRoot -SourceOnly | ConvertFrom-Json

        $result.result | Should -Be "pass"
        $result.package_source_file_count | Should -Be 7
        $result.coverage_anchor_count | Should -BeGreaterOrEqual 16
        $result.external_reference_license_count | Should -Be 2
        $result.forbidden_authority_count | Should -Be 0
        $result.private_canary_count | Should -Be 0
    }

    It "rejects forbidden package authority <Name>" -TestCases @(
        @{ Name = "process"; Source = 'fn drift() { let _ = std::process::Command::new("cmd.exe"); }' }
        @{ Name = "network"; Source = 'const DRIFT: &str = "https://example.invalid";' }
        @{ Name = "generic-extraction"; Source = 'fn drift() { let _ = zip::ZipArchive::new(()); }' }
        @{ Name = "plugin"; Source = 'const DRIFT: &str = "plugin";' }
        @{ Name = "ui"; Source = 'fn drift() { let _ = slint::Timer::default(); }' }
    ) {
        param($Name, $Source)
        $fixture = New-BackupAuditFixture -Name "authority-$Name"
        Add-Content -LiteralPath (Join-Path $fixture "crates\state\src\package\mod.rs") `
            -Value $Source

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-BACKUP-FORBIDDEN-AUTHORITY*"
    }

    It "rejects loss of an adversarial coverage anchor" {
        $fixture = New-BackupAuditFixture -Name "missing-mutation-matrix"
        $path = Join-Path $fixture "crates\state\tests\fault_matrix_contract.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'every_package_prefix_and_one_bit_mutation_fails_closed',
            'coverage_removed'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-BACKUP-TEST-MATRIX*"
    }

    It "rejects loss of the application recovery gate" {
        $fixture = New-BackupAuditFixture -Name "missing-app-recovery"
        Remove-Item -LiteralPath `
            (Join-Path $fixture "crates\app\tests\recovery_adversarial_contract.rs")

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-BACKUP-MISSING-BOUNDARY*"
    }

    It "rejects private canaries in production backup surfaces" {
        $fixture = New-BackupAuditFixture -Name "private-canary"
        Add-Content -LiteralPath (Join-Path $fixture "crates\state\src\package\mod.rs") `
            -Value 'const LEAK: &str = "C:\private\codex-home";'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-BACKUP-PRIVATE-CANARY*"
    }

    It "rejects external reference license drift" {
        $fixture = New-BackupAuditFixture -Name "license-drift"
        $path = Join-Path $fixture "third_party\UPSTREAM.toml"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'license = "MIT"',
            'license = "Proprietary"'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-BACKUP-UPSTREAM-LICENSE*"
    }

    It "rejects a missing external license notice" {
        $fixture = New-BackupAuditFixture -Name "missing-license"
        Remove-Item -LiteralPath `
            (Join-Path $fixture "third_party\licenses\ccusage-MIT.txt")

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-BACKUP-MISSING-BOUNDARY*"
    }

    It "rejects exact dependency policy drift" {
        $mutatedAudit = Join-Path $TestDrive "audit-dependency-drift.ps1"
        $text = [System.IO.File]::ReadAllText($Audit)
        $text = [regex]::Replace(
            $text,
            '(?m)(^\$expectedDependencyPolicySha256\s*=\s*'')[0-9a-f]{64}(''\s*$)',
            { param($match) $match.Groups[1].Value + ('0' * 64) + $match.Groups[2].Value }
        )
        [System.IO.File]::WriteAllText($mutatedAudit, $text)

        { & $mutatedAudit -RepositoryRoot $RepositoryRoot } |
            Should -Throw "*TM-BACKUP-DEPENDENCY-POLICY*"
    }

    It "rejects exact resolved feature policy drift" {
        $mutatedAudit = Join-Path $TestDrive "audit-feature-drift.ps1"
        $text = [System.IO.File]::ReadAllText($Audit)
        $text = [regex]::Replace(
            $text,
            '(?m)(^\$expectedFeaturePolicySha256\s*=\s*'')[0-9a-f]{64}(''\s*$)',
            { param($match) $match.Groups[1].Value + ('0' * 64) + $match.Groups[2].Value }
        )
        [System.IO.File]::WriteAllText($mutatedAudit, $text)

        { & $mutatedAudit -RepositoryRoot $RepositoryRoot } |
            Should -Throw "*TM-BACKUP-FEATURE-POLICY*"
    }

    It "rejects loss of the synthetic exported archive privacy proof" {
        $fixture = New-BackupAuditFixture -Name "missing-archive-privacy"
        $path = Join-Path $fixture "crates\state\tests\package_adversarial_contract.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'synthetic_exported_archive_is_free_of_private_input_canaries',
            'privacy_proof_removed'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-BACKUP-TEST-MATRIX*"
    }
}
