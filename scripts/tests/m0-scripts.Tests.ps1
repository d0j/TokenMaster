Describe "TokenMaster M0 script contracts" {
    BeforeAll {
        $ScriptsRoot = Split-Path -Parent $PSScriptRoot
        $RepositoryRoot = (Resolve-Path (Join-Path $ScriptsRoot "..")).Path
    }

    It "<Name> exists and is fail-fast" -TestCases @(
        @{ Name = "verify-m0.ps1" }
        @{ Name = "package-product.ps1" }
    ) {
        param([string]$Name)

        $Path = Join-Path $ScriptsRoot $Name
        (Test-Path -LiteralPath $Path) | Should -Be $true
        $Text = Get-Content -LiteralPath $Path -Raw
        $Text | Should -Match "Set-StrictMode -Version Latest"
        $Text | Should -Match '\$ErrorActionPreference = "Stop"'
        $Text | Should -Match '\[string\]\$RepositoryRoot'
        { [void][scriptblock]::Create($Text) } | Should -Not -Throw
        $Text | Should -Not -Match '"[^"\r\n]*\$[A-Za-z_][A-Za-z0-9_]*:'
    }

    It "pins and bootstraps the Pester source used by GitHub Actions" {
        $Workflow = Get-Content -LiteralPath (Join-Path $RepositoryRoot ".github\workflows\tokenmaster-m0-windows.yml") -Raw
        $Workflow | Should -Match 'Get-PSRepository -Name PSGallery -ErrorAction SilentlyContinue'
        $Workflow | Should -Match 'Register-PSRepository -Default'
        $Workflow | Should -Match 'Install-Module Pester -RequiredVersion 5\.7\.1'
    }

    It "runs the baseline when any GitHub workflow changes" {
        $Workflow = Get-Content -LiteralPath (Join-Path $RepositoryRoot ".github\workflows\tokenmaster-m0-windows.yml") -Raw
        $Workflow | Should -Match '(?m)^\s+-\s+"\.github/workflows/\*\*"\s*$'
    }

    It "allows the serialized Windows M0 receipt sufficient wall time" {
        $Workflow = Get-Content -LiteralPath (Join-Path $RepositoryRoot ".github\workflows\tokenmaster-m0-windows.yml") -Raw
        $Workflow | Should -Match '(?m)^\s+timeout-minutes:\s+75\s*$'
    }

    It "verification uses the root locked workspace and labels external gates" {
        $Text = Get-Content -LiteralPath (Join-Path $ScriptsRoot "verify-m0.ps1") -Raw
        $Text | Should -Match 'RequiredPesterVersion = \[version\]"5\.7\.1"'
        $Text | Should -Match 'Import-Module Pester -RequiredVersion \$RequiredPesterVersion'
        $Text | Should -Match 'Join-Path \$RepositoryRoot "Cargo\.toml"'
        $Text | Should -Not -Match "tokenmaster[\\/]Cargo.toml"
        $Text | Should -Match "--locked"
        $Text | Should -Match 'audit-clean-root\.ps1'
        $Text | Should -Match 'RUSTFLAGS'
        $Text | Should -Not -Match '--", "-D", "warnings"'
        $Text | Should -Match "interactive"
        $Text | Should -Match "24-hour"
        $Text | Should -Not -Match "cargo test --workspace"
    }

    It "verification emits bounded stage progress without command details" {
        $Text = Get-Content -LiteralPath (Join-Path $ScriptsRoot "verify-m0.ps1") -Raw
        $Text | Should -Match "TM-M0-STAGE-BEGIN"
        $Text | Should -Match "TM-M0-STAGE-PASS"
        $Text | Should -Not -Match 'Write-Host.*\$Arguments'
    }

    It "verification resolves cargo as an application and targets the shipped MSVC binary" {
        $Text = Get-Content -LiteralPath (Join-Path $ScriptsRoot "verify-m0.ps1") -Raw
        $Text | Should -Match 'Get-Command cargo\.exe -CommandType Application'
        $Text | Should -Match 'x86_64-pc-windows-msvc'
        $Text | Should -Match '"-p", "tokenmaster-app", "--release"'
        $Text | Should -Not -Match 'mingw'
        $Text | Should -Not -Match 'windows-gnu'
        $Text | Should -Not -Match 'tokenmaster-m0'
        $Text | Should -Not -Match 'CARGO_BUILD_JOBS'
    }

    It "uses immutable commits for the current Node 24 GitHub Actions majors" {
        $Workflow = Get-Content -LiteralPath (Join-Path $RepositoryRoot ".github\workflows\tokenmaster-m0-windows.yml") -Raw
        $Workflow | Should -Match 'actions/checkout@[0-9a-f]{40} # v7'
        $Workflow | Should -Match 'actions/upload-artifact@[0-9a-f]{40} # v7'
        $Workflow | Should -Not -Match 'actions/(checkout|upload-artifact)@v4'
    }

    It "verification has no foreign runtime or predecessor oracle dependency" {
        $Text = Get-Content -LiteralPath (Join-Path $ScriptsRoot "verify-m0.ps1") -Raw
        $Text | Should -Not -Match '(?i)\bgo\.exe\b|\bnode\.exe\b|\bpython\.exe\b'
    }

    It "developer verification receipt excludes command arguments and command output" {
        $Text = Get-Content -LiteralPath (Join-Path $ScriptsRoot "verify-m0.ps1") -Raw
        $Text | Should -Not -Match 'arguments = \$Arguments'
        $Text | Should -Not -Match 'rust = \(& \$Rustc'
        $Text | Should -Not -Match 'mingw = \$MingwVersion'
    }

    It "packaging resolves an absolute repository root and uses no foreign runtime" {
        $Text = Get-Content -LiteralPath (Join-Path $ScriptsRoot "package-product.ps1") -Raw
        $Text | Should -Match '\[IO\.Path\]::GetFullPath\(\(Resolve-Path -LiteralPath \$RepositoryRoot\)\.Path\)'
        $Text | Should -Not -Match '(?i)\bgo\.exe\b|\bnode\.exe\b|\bpython\.exe\b'
    }

    It "verification runs the surviving script contracts" {
        $Text = Get-Content -LiteralPath (Join-Path $ScriptsRoot "verify-m0.ps1") -Raw
        $Text | Should -Match 'm0-scripts\.Tests\.ps1'
        $Text | Should -Match 'immutable-actions\.Tests\.ps1'
        $Text | Should -Match 'release-artifact-workflow\.Tests\.ps1'
        $Text | Should -Match 'dependency-policy\.Tests\.ps1'
        $Text | Should -Match 'validate-immutable-actions\.ps1'
        $Text | Should -Match 'verify-dependency-policy\.ps1'
    }
}
