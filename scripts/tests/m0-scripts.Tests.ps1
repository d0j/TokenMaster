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

    # A wall-clock deadline in a test bounds a hang; it does not assert latency, and a
    # latency claim is made by comparing a measured elapsed time, not by failing to reach
    # a timeout. Short budgets measure the runner instead of the product: fifty of these
    # existed, forty-nine under thirty seconds and ten at two, and two separate CI runs
    # died on them. Raising a bound costs nothing on the green path, because the deadline
    # only elapses when something is already broken.
    It "gives every test hang bound at least thirty seconds" {
        $offenders = @()
        Get-ChildItem -LiteralPath (Join-Path $RepositoryRoot 'crates') -Recurse -File -Filter '*.rs' |
            Where-Object { $_.FullName -match '\\tests\\' } |
            ForEach-Object {
                $text = Get-Content -LiteralPath $_.FullName -Raw
                foreach ($match in [regex]::Matches(
                    $text, 'Instant::now\(\)\s*\+\s*Duration::from_secs\((\d+)\)'
                )) {
                    if ([int]$match.Groups[1].Value -lt 30) {
                        $offenders += "$($_.Name): $($match.Value)"
                    }
                }
            }
        $offenders -join '; ' | Should -BeExactly ''
    }

    # A push touching only one of these files changed what the build produces while
    # triggering nothing, so the change landed ungated. `.gitattributes` is the case
    # that occurred: it decides the bytes `include_str!` compiles in.
    It "triggers on every root file that decides what the build produces" {
        $Workflow = Get-Content -LiteralPath (
            Join-Path $RepositoryRoot ".github\workflows\tokenmaster-m0-windows.yml"
        ) -Raw
        # No `s` flag: with it, `.` swallows newlines and both blocks merge into one.
        $triggers = [regex]::Matches($Workflow, '(?m)^ {4}paths:\r?\n(?<body>(?: {6}-[^\r\n]*\r?\n)+)')
        $triggers.Count | Should -Be 2
        foreach ($trigger in $triggers) {
            foreach ($path in @(
                'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'deny.toml',
                '.gitattributes', '.gitleaks.toml', '.gitleaksignore'
            )) {
                $trigger.Groups['body'].Value | Should -Match ([regex]::Escape("- `"$path`""))
            }
        }
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

    It "verification enumerates its Pester suites instead of naming them" {
        $Text = Get-Content -LiteralPath (Join-Path $ScriptsRoot "verify-m0.ps1") -Raw
        # Every suite on disk is run by construction now, so what is guarded here is that it
        # stays that way. Two shapes failed before: a hand-written list that named seven of
        # eight, and then a substring match over this script's text, which a `#` in front of a
        # stage satisfied from inside the comment it created. A literal suite path in an
        # Invoke-PesterChecked call is that shape returning, so it is forbidden outright.
        $Text | Should -Match '(?s)Get-ChildItem.{0,200}?\*\.Tests\.ps1'
        $Text | Should -Not -Match 'Invoke-PesterChecked[^\r\n]*\.Tests\.ps1'
        $Suites = @(Get-ChildItem -LiteralPath (Join-Path $ScriptsRoot "tests") `
                -Filter "*.Tests.ps1" -File)
        $Suites.Count | Should -BeGreaterOrEqual 8
        $Text | Should -Match 'validate-immutable-actions\.ps1'
        $Text | Should -Match 'verify-dependency-policy\.ps1'
    }
}
