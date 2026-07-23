Describe "TokenMaster M0 script contracts" {
    BeforeAll {
        $ScriptsRoot = Split-Path -Parent $PSScriptRoot
        $RepositoryRoot = (Resolve-Path (Join-Path $ScriptsRoot "..")).Path
    }

    It "<Name> exists and is fail-fast" -TestCases @(
        @{ Name = "verify-m0.ps1" }
        @{ Name = "run-m0-soak.ps1" }
        @{ Name = "package-m0.ps1" }
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

    It "pins the Pester version used by GitHub Actions" {
        $Workflow = Get-Content -LiteralPath (Join-Path $RepositoryRoot ".github\workflows\tokenmaster-m0-windows.yml") -Raw
        $Workflow | Should -Match 'Install-Module Pester -RequiredVersion 5\.7\.1'
    }

    It "runs the baseline when any GitHub workflow changes" {
        $Workflow = Get-Content -LiteralPath (Join-Path $RepositoryRoot ".github\workflows\tokenmaster-m0-windows.yml") -Raw
        $Workflow | Should -Match '(?m)^\s+-\s+"\.github/workflows/\*\*"\s*$'
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

    It "verification preflights the external GNU linker and Windows import library" {
        $Text = Get-Content -LiteralPath (Join-Path $ScriptsRoot "verify-m0.ps1") -Raw
        $Text | Should -Match 'x86_64-w64-mingw32-gcc\.exe'
        $Text | Should -Match 'C:\\mingw64'
        $Text | Should -Match 'C:\\msys64\\mingw64'
        $Text | Should -Match 'libshlwapi\.a'
        $Text | Should -Match 'CommandType Application'
        $Text | Should -Match 'Get-Command cargo\.exe -CommandType Application'
        $Text | Should -Match 'CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER'
        $Text | Should -Match '& \$MingwLinker --version'
        $Text | Should -Not -Match 'Get-Command "x86_64-w64-mingw32-gcc\.exe"'
    }

    It "serializes Cargo work for deterministic Windows GNU linking" {
        $Text = Get-Content -LiteralPath (Join-Path $ScriptsRoot "verify-m0.ps1") -Raw
        $Text | Should -Match '\$env:CARGO_BUILD_JOBS = "1"'
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

    It "soak and packaging use root-only paths" {
        foreach ($Name in @("run-m0-soak.ps1", "package-m0.ps1")) {
            $Text = Get-Content -LiteralPath (Join-Path $ScriptsRoot $Name) -Raw
            $Text | Should -Match 'Join-Path \$RepositoryRoot "Cargo\.toml"|Join-Path \$RepositoryRoot "target\\x86_64-pc-windows-gnu\\release\\tokenmaster-m0\.exe"'
            $Text | Should -Not -Match 'tokenmaster[\\/]'
            $Text | Should -Not -Match '(?i)\bgo\.exe\b|\bnode\.exe\b|\bpython\.exe\b'
        }
    }

    It "verification runs both script and soak helper contracts" {
        $Text = Get-Content -LiteralPath (Join-Path $ScriptsRoot "verify-m0.ps1") -Raw
        $Text | Should -Match 'm0-scripts\.Tests\.ps1'
        $Text | Should -Match 'm0-soak-lib\.Tests\.ps1'
        $Text | Should -Match 'immutable-actions\.Tests\.ps1'
        $Text | Should -Match 'release-artifact-workflow\.Tests\.ps1'
        $Text | Should -Match 'dependency-policy\.Tests\.ps1'
        $Text | Should -Match 'validate-immutable-actions\.ps1'
        $Text | Should -Match 'verify-dependency-policy\.ps1'
    }

    It "packaging rejects a dirty tree and never claims a release" {
        $Text = Get-Content -LiteralPath (Join-Path $ScriptsRoot "package-m0.ps1") -Raw
        $Text | Should -Match "git status --porcelain"
        $Text | Should -Match "M0 architecture proof"
        $Text | Should -Not -Match '"released"'
        $Text | Should -Match "GetFullPath"
    }

    It "packaging binds every external receipt to the current commit and executable" {
        $Text = Get-Content -LiteralPath (Join-Path $ScriptsRoot "package-m0.ps1") -Raw
        $Text | Should -Match 'Receipt\.commit -ne \$Commit'
        $Text | Should -Match 'Receipt\.dirty'
        $Text | Should -Match 'Receipt\.executableSha256 -ne \$ExecutableSha256'
    }

    It "soak acceptance uses wall-clock completion" {
        $Text = Get-Content -LiteralPath (Join-Path $ScriptsRoot "run-m0-soak.ps1") -Raw
        $Text | Should -Match "wallHours"
        $Text | Should -Match 'WallHours -ge \$DurationHours'
    }

    It "soak samples are appended and JSON summaries are atomic" {
        $Text = Get-Content -LiteralPath (Join-Path $ScriptsRoot "run-m0-soak.ps1") -Raw
        $Text | Should -Match 'Write-SoakCsvSample'
        $Text | Should -Match 'Write-AtomicJson'
        $Text | Should -Not -Match '\$Samples\s*\|\s*Export-Csv'
    }

    It "soak evaluates drift gaps and every bounded Windows counter" {
        $Text = Get-Content -LiteralPath (Join-Path $ScriptsRoot "run-m0-soak.ps1") -Raw
        foreach ($Marker in @(
            "privateSlopeMiBPerHour",
            "maxSampleGapSeconds",
            "handleDelta",
            "threadDelta",
            "userObjectDelta",
            "gdiObjectDelta"
        )) {
            $Text | Should -Match $Marker
        }
        $Text | Should -Match 'Get-ProcessGuiResources'
    }

    It "only a full M0 soak can publish the canonical receipt" {
        $Text = Get-Content -LiteralPath (Join-Path $ScriptsRoot "run-m0-soak.ps1") -Raw
        $Text | Should -Match 'soak-24h\.json'
        $Text | Should -Match 'DurationHours -ge 24\.0'
        $Text | Should -Match 'Result -eq "pass"'
    }

    It "starts the measured wall-clock window after bounded warm-up" {
        $Text = Get-Content -LiteralPath (Join-Path $ScriptsRoot "run-m0-soak.ps1") -Raw
        $Text | Should -Match 'ValidateRange\(0, 300\).*WarmupSeconds'
        $WarmupIndex = $Text.IndexOf('Start-Sleep -Seconds $WarmupSeconds')
        $StartedIndex = $Text.IndexOf('$Started = [DateTimeOffset]::UtcNow')
        ($WarmupIndex -ge 0) | Should -Be $true
        ($StartedIndex -gt $WarmupIndex) | Should -Be $true
    }

    It "binds a full soak to one clean commit and executable hash" {
        $Text = Get-Content -LiteralPath (Join-Path $ScriptsRoot "run-m0-soak.ps1") -Raw
        $Text | Should -Match 'git rev-parse HEAD'
        $Text | Should -Match 'git status --porcelain'
        $Text | Should -Match 'Get-FileHash.*SHA256'
        $Text | Should -Match 'DurationHours -ge 24\.0 -and \$Dirty'
        $Text | Should -Match 'executableSha256'
    }
}
