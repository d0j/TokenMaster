Describe "TokenMaster clean-root audit" {
    BeforeAll {
        $ScriptsRoot = Split-Path -Parent $PSScriptRoot
        $Audit = Join-Path $ScriptsRoot "audit-clean-root.ps1"
    }

    It "rejects a second Rust workspace with a stable marker" {
        Set-Content -LiteralPath (Join-Path $TestDrive "Cargo.toml") -Value "[workspace]" -Encoding utf8
        $Nested = Join-Path $TestDrive "experimental"
        New-Item -ItemType Directory -Path $Nested -Force | Out-Null
        Set-Content -LiteralPath (Join-Path $Nested "Cargo.toml") -Value "[workspace]" -Encoding utf8

        { & $Audit -RepositoryRoot $TestDrive } | Should -Throw "*TM-CLEAN-SECOND-WORKSPACE*"
    }

    It "accepts the project root without exposing arbitrary paths" {
        $RepositoryRoot = (Resolve-Path (Join-Path $ScriptsRoot "..")).Path
        $Output = & $Audit -RepositoryRoot $RepositoryRoot

        $Output | Should -Be "TM-CLEAN-PASS"
    }
}
