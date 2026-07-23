Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Describe 'P3-E interactive receipt validator' {
    BeforeAll {
        $ScriptsRoot = Split-Path -Parent $PSScriptRoot
        $Validator = Join-Path $ScriptsRoot 'validate-p3e-interactive.ps1'
        . (Join-Path $ScriptsRoot 'product-package-lib.ps1')
        $ScenarioNames = @(
            'tray_show_hide_quit', 'explorer_restart', 'secondary_activation', 'hotkey_registered',
            'hotkey_conflict', 'startup_enable_readback_signin_disable',
            'startup_relocation_repair_remove', 'startup_access_denied', 'lock_unlock',
            'sleep_resume', 'rapid_show_hide_mode'
        )

        function New-P3eFixture {
            $root = Join-Path ([IO.Path]::GetTempPath()) ('tokenmaster-p3e-' + [Guid]::NewGuid().ToString('N'))
            New-Item -ItemType Directory -Path $root | Out-Null
            Push-Location $root
            try {
                & git.exe init --quiet
                & git.exe config user.email 'p3e@example.invalid'
                & git.exe config user.name 'P3E Test'
                [IO.File]::WriteAllText((Join-Path $root 'README.md'), 'fixture')
                $exe = Join-Path $root 'tokenmaster.exe'
                [IO.File]::WriteAllBytes($exe, [byte[]](1, 2, 3, 4))
                & git.exe add README.md
                & git.exe add tokenmaster.exe
                & git.exe commit --quiet -m 'fixture'
            }
            finally { Pop-Location }
            $commit = (& git.exe -C $root rev-parse HEAD).Trim()
            $packageRoot = Join-Path ([IO.Path]::GetTempPath()) (
                'tokenmaster-p3e-package-' + [Guid]::NewGuid().ToString('N')
            )
            $stage = Join-Path $packageRoot 'TokenMaster-0.1.0-windows-x64'
            New-Item -ItemType Directory -Path $stage -Force | Out-Null
            $executable = Join-Path $root 'tokenmaster.exe'
            Copy-Item -LiteralPath $executable -Destination (Join-Path $stage 'TokenMaster.exe')
            [IO.File]::WriteAllBytes((Join-Path $stage 'tokenmaster.portable'), [byte[]]@())
            foreach ($name in @('README.md', 'README_RU.md', 'LICENSE')) {
                Set-Content -LiteralPath (Join-Path $stage $name) -Value $name -NoNewline
            }
            Set-Content -LiteralPath (Join-Path $stage 'THIRD_PARTY_NOTICES.txt') `
                -Value 'dependency 1.0 | MIT | https://example.invalid/dependency' -NoNewline
            $executableHash = (Get-FileHash -LiteralPath $executable -Algorithm SHA256).
                Hash.ToLowerInvariant()
            [ordered]@{
                schemaVersion = 1
                status = 'unsigned package candidate'
                version = '0.1.0'
                commit = $commit
                target = 'x86_64-pc-windows-msvc'
                executableSha256 = $executableHash
            } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $stage 'BUILDINFO.json') `
                -Encoding utf8NoBOM
            [ordered]@{
                bomFormat = 'CycloneDX'
                specVersion = '1.6'
                version = 1
                components = @([ordered]@{
                    type = 'library'
                    name = 'dependency'
                    version = '1.0'
                    licenses = @([ordered]@{ expression = 'MIT' })
                    purl = 'pkg:cargo/dependency@1.0'
                })
            } | ConvertTo-Json -Depth 8 | Set-Content `
                -LiteralPath (Join-Path $stage 'SBOM.cdx.json') -Encoding utf8NoBOM
            Write-ProductChecksums -StagePath $stage
            $package = Join-Path $packageRoot 'TokenMaster-0.1.0-windows-x64-unsigned.zip'
            New-DeterministicZip -StagePath $stage -DestinationPath $package
            $packageReceipt = Join-Path $packageRoot `
                'TokenMaster-0.1.0-windows-x64-unsigned.receipt.json'
            [ordered]@{
                schemaVersion = 1
                status = 'unsigned package candidate'
                version = '0.1.0'
                commit = $commit
                executableSha256 = $executableHash
                packageSha256 = (Get-FileHash -LiteralPath $package -Algorithm SHA256).
                    Hash.ToLowerInvariant()
                packageFile = [IO.Path]::GetFileName($package)
            } | ConvertTo-Json | Set-Content -LiteralPath $packageReceipt -Encoding utf8NoBOM
            [pscustomobject]@{
                Root = $root
                Executable = $executable
                Receipt = Join-Path ([IO.Path]::GetTempPath()) (
                    'tokenmaster-p3e-' + [Guid]::NewGuid().ToString('N') + '.json'
                )
                PackageRoot = $packageRoot
                Package = $package
                PackageReceipt = $packageReceipt
            }
        }

        function New-ValidReceipt([object]$Fixture) {
            $commit = (& git.exe -C $Fixture.Root rev-parse HEAD).Trim()
            [ordered]@{
                schema = 'tokenmaster.p3e.interactive.v1'
                result = 'pass'
                commit = $commit
                dirty = $false
                executableKind = 'packaged-production'
                executableSha256 = (Get-FileHash -LiteralPath $Fixture.Executable -Algorithm SHA256).Hash.ToLowerInvariant()
                disposableHost = $true
                rollback = [ordered]@{ registryPreStateRestored = $true; processesStopped = $true }
                scenarios = @($ScenarioNames | ForEach-Object { [ordered]@{ name = $_; result = 'pass' } })
                resources = [ordered]@{
                    warmupCycles = 8; measuredCycles = 64; privateGrowthMiB = 8.0
                    handleDelta = 0; threadDelta = 0; userObjectDelta = 0; gdiObjectDelta = 0
                }
            }
        }

        function Invoke-P3eValidator([object]$Fixture, [object]$Receipt) {
            $Receipt | ConvertTo-Json -Depth 8 -Compress | Set-Content -LiteralPath $Fixture.Receipt -Encoding utf8NoBOM
            $output = & $Validator -RepositoryRoot $Fixture.Root `
                -ReceiptPath $Fixture.Receipt `
                -ExecutablePath $Fixture.Executable `
                -PackagePath $Fixture.Package `
                -PackageReceiptPath $Fixture.PackageReceipt
            [pscustomobject]@{ ExitCode = $LASTEXITCODE; Output = ($output -join "`n") }
        }
    }

    AfterEach {
        if ($script:fixture) {
            Remove-Item -LiteralPath $script:fixture.Root -Recurse -Force
            Remove-Item -LiteralPath $script:fixture.PackageRoot -Recurse -Force
            Remove-Item -LiteralPath $script:fixture.Receipt -Force -ErrorAction SilentlyContinue
            $script:fixture = $null
        }
    }

    It 'preflights the exact clean operator-attested receipt without paths' {
        $script:fixture = New-P3eFixture
        $result = Invoke-P3eValidator $script:fixture (New-ValidReceipt $script:fixture)
        $result.ExitCode | Should -Be 0
        $result.Output | Should -Be '{"result":"preflight-pass","schema":"tokenmaster.p3e.interactive.v1"}'
        $result.Output | Should -Not -Match '(?i)[a-z]:[\\/]|\\\\'
    }

    It 'rejects package, package receipt, or tested executable provenance drift' -TestCases @(
        @{ Mutation = { param($f) Add-Content -LiteralPath $f.Package -Value 'drift' } }
        @{ Mutation = {
            param($f)
            $value = Get-Content -LiteralPath $f.PackageReceipt -Raw | ConvertFrom-Json
            $value.packageSha256 = '0' * 64
            $value | ConvertTo-Json | Set-Content -LiteralPath $f.PackageReceipt -Encoding utf8NoBOM
        } }
        @{ Mutation = {
            param($f)
            $value = Get-Content -LiteralPath $f.PackageReceipt -Raw | ConvertFrom-Json
            $value.schemaVersion = '1'
            $value | ConvertTo-Json | Set-Content -LiteralPath $f.PackageReceipt -Encoding utf8NoBOM
        } }
        @{ Mutation = { param($f) [IO.File]::WriteAllBytes($f.Executable, [byte[]](9, 9, 9)) } }
    ) {
        param($Mutation)
        $script:fixture = New-P3eFixture
        & $Mutation $script:fixture
        (Invoke-P3eValidator $script:fixture (New-ValidReceipt $script:fixture)).ExitCode |
            Should -Be 1
    }

    It 'rejects dirty worktrees and identity or executable hash mismatches' -TestCases @(
        @{ Mutation = { param($r) $r.dirty = $true } }
        @{ Mutation = { param($r) $r.commit = '0000000000000000000000000000000000000000' } }
        @{ Mutation = { param($r) $r.executableSha256 = '0' * 64 } }
    ) {
        param($Mutation)
        $script:fixture = New-P3eFixture
        $receipt = New-ValidReceipt $script:fixture
        & $Mutation $receipt
        (Invoke-P3eValidator $script:fixture $receipt).ExitCode | Should -Be 1
    }

    It 'rejects an actually dirty current worktree' {
        $script:fixture = New-P3eFixture
        [IO.File]::WriteAllText((Join-Path $script:fixture.Root 'untracked.txt'), 'dirty')
        (Invoke-P3eValidator $script:fixture (New-ValidReceipt $script:fixture)).ExitCode | Should -Be 1
    }

    It 'rejects missing extra or duplicate scenarios and failed scenario results' -TestCases @(
        @{ Mutation = { param($r) $r.scenarios = @($r.scenarios | Select-Object -Skip 1) } }
        @{ Mutation = { param($r) $r.scenarios += [ordered]@{ name = 'tray_show_hide_quit'; result = 'pass' } } }
        @{ Mutation = { param($r) $r.scenarios[0].result = 'fail' } }
    ) {
        param($Mutation)
        $script:fixture = New-P3eFixture
        $receipt = New-ValidReceipt $script:fixture
        & $Mutation $receipt
        (Invoke-P3eValidator $script:fixture $receipt).ExitCode | Should -Be 1
    }

    It 'rejects unsafe host rollback, insufficient cycles, and resource overages' -TestCases @(
        @{ Mutation = { param($r) $r.disposableHost = $false } }
        @{ Mutation = { param($r) $r.rollback.processesStopped = $false } }
        @{ Mutation = { param($r) $r.resources.warmupCycles = 7 } }
        @{ Mutation = { param($r) $r.resources.measuredCycles = 63 } }
        @{ Mutation = { param($r) $r.resources.privateGrowthMiB = 8.1 } }
        @{ Mutation = { param($r) $r.resources.handleDelta = 1 } }
    ) {
        param($Mutation)
        $script:fixture = New-P3eFixture
        $receipt = New-ValidReceipt $script:fixture
        & $Mutation $receipt
        (Invoke-P3eValidator $script:fixture $receipt).ExitCode | Should -Be 1
    }

    It 'rejects privacy leaks and malformed JSON or types' -TestCases @(
        @{ Raw = $null; Mutation = { param($r) $r.note = 'C:\\Users\\secret' } }
        @{ Raw = $null; Mutation = { param($r) $r.rollback.note = 'extra' } }
        @{ Raw = '{'; Mutation = $null }
        @{ Raw = $null; Mutation = { param($r) $r.dirty = 'false' } }
    ) {
        param($Raw, $Mutation)
        $script:fixture = New-P3eFixture
        if ($null -ne $Raw) {
            Set-Content -LiteralPath $script:fixture.Receipt -Value $Raw -Encoding utf8NoBOM
            $output = & $Validator -RepositoryRoot $script:fixture.Root `
                -ReceiptPath $script:fixture.Receipt `
                -ExecutablePath $script:fixture.Executable `
                -PackagePath $script:fixture.Package `
                -PackageReceiptPath $script:fixture.PackageReceipt
            $result = [pscustomobject]@{ ExitCode = $LASTEXITCODE; Output = ($output -join "`n") }
        }
        else {
            $receipt = New-ValidReceipt $script:fixture
            & $Mutation $receipt
            $result = Invoke-P3eValidator $script:fixture $receipt
        }
        $result.ExitCode | Should -Be 1
        $result.Output | Should -Not -Match '(?i)[a-z]:[\\/]|\\\\|secret'
    }

    It 'rejects duplicate JSON properties, oversized receipts, fractional native counters, and the M0 executable name' -TestCases @(
        @{ Kind = 'duplicate' }
        @{ Kind = 'oversized' }
        @{ Kind = 'fractional' }
        @{ Kind = 'm0-name' }
    ) {
        param($Kind)
        $script:fixture = New-P3eFixture
        $receipt = New-ValidReceipt $script:fixture
        if ($Kind -eq 'duplicate') {
            $raw = $receipt | ConvertTo-Json -Depth 8 -Compress
            $raw = $raw -replace '^\{"schema"', '{"result":"fail","schema"'
            Set-Content -LiteralPath $script:fixture.Receipt -Value $raw -Encoding utf8NoBOM
            $output = & $Validator -RepositoryRoot $script:fixture.Root `
                -ReceiptPath $script:fixture.Receipt `
                -ExecutablePath $script:fixture.Executable `
                -PackagePath $script:fixture.Package `
                -PackageReceiptPath $script:fixture.PackageReceipt
            $result = [pscustomobject]@{ ExitCode = $LASTEXITCODE; Output = ($output -join "`n") }
        }
        elseif ($Kind -eq 'oversized') {
            Set-Content -LiteralPath $script:fixture.Receipt -Value (' ' * 32769) -Encoding utf8NoBOM
            $output = & $Validator -RepositoryRoot $script:fixture.Root `
                -ReceiptPath $script:fixture.Receipt `
                -ExecutablePath $script:fixture.Executable `
                -PackagePath $script:fixture.Package `
                -PackageReceiptPath $script:fixture.PackageReceipt
            $result = [pscustomobject]@{ ExitCode = $LASTEXITCODE; Output = ($output -join "`n") }
        }
        elseif ($Kind -eq 'fractional') {
            $receipt.resources.handleDelta = -0.1
            $result = Invoke-P3eValidator $script:fixture $receipt
        }
        else {
            $renamed = Join-Path $script:fixture.Root 'tokenmaster-m0.exe'
            Move-Item -LiteralPath $script:fixture.Executable -Destination $renamed
            $script:fixture.Executable = $renamed
            $receipt.executableSha256 = (Get-FileHash -LiteralPath $renamed -Algorithm SHA256).Hash.ToLowerInvariant()
            $result = Invoke-P3eValidator $script:fixture $receipt
        }
        $result.ExitCode | Should -Be 1
        $result.Output | Should -Not -Match '(?i)[a-z]:[\\/]|\\\\'
    }

    It 'ignores hostile Git environment and never emits native stderr' {
        $script:fixture = New-P3eFixture
        $receipt = New-ValidReceipt $script:fixture
        $priorGitDir = $env:GIT_DIR
        try {
            $env:GIT_DIR = 'C:\p3e-secret\.git'
            $result = Invoke-P3eValidator $script:fixture $receipt
        }
        finally { $env:GIT_DIR = $priorGitDir }
        $result.ExitCode | Should -Be 0
        $result.Output | Should -Not -Match '(?i)[a-z]:[\\/]|\\\\|fatal|secret'
    }

    It 'cannot hide untracked files through local or environment Git config' -TestCases @(
        @{ Kind = 'local' }
        @{ Kind = 'environment' }
    ) {
        param($Kind)
        $script:fixture = New-P3eFixture
        $receipt = New-ValidReceipt $script:fixture
        [IO.File]::WriteAllText((Join-Path $script:fixture.Root 'untracked.txt'), 'dirty')
        if ($Kind -eq 'local') {
            & git.exe -C $script:fixture.Root config status.showUntrackedFiles no
            $result = Invoke-P3eValidator $script:fixture $receipt
        }
        else {
            $priorCount = $env:GIT_CONFIG_COUNT
            $priorKey = $env:GIT_CONFIG_KEY_0
            $priorValue = $env:GIT_CONFIG_VALUE_0
            try {
                $env:GIT_CONFIG_COUNT = '1'
                $env:GIT_CONFIG_KEY_0 = 'status.showUntrackedFiles'
                $env:GIT_CONFIG_VALUE_0 = 'no'
                $result = Invoke-P3eValidator $script:fixture $receipt
            }
            finally {
                $env:GIT_CONFIG_COUNT = $priorCount
                $env:GIT_CONFIG_KEY_0 = $priorKey
                $env:GIT_CONFIG_VALUE_0 = $priorValue
            }
        }
        $result.ExitCode | Should -Be 1
    }

    It 'rejects a nested directory instead of accepting its parent repository' {
        $script:fixture = New-P3eFixture
        $receipt = New-ValidReceipt $script:fixture
        $nested = Join-Path $script:fixture.Root 'nested'
        New-Item -ItemType Directory -Path $nested | Out-Null
        $receipt | ConvertTo-Json -Depth 8 -Compress | Set-Content -LiteralPath $script:fixture.Receipt -Encoding utf8NoBOM
        $output = & $Validator -RepositoryRoot $nested `
            -ReceiptPath $script:fixture.Receipt `
            -ExecutablePath $script:fixture.Executable `
            -PackagePath $script:fixture.Package `
            -PackageReceiptPath $script:fixture.PackageReceipt
        $LASTEXITCODE | Should -Be 1
        ($output -join "`n") | Should -Not -Match '(?i)[a-z]:[\\/]|\\\\'
    }

    It 'uses no optional Git locks and explicit untracked and submodule status modes' {
        $text = Get-Content -LiteralPath $Validator -Raw
        $text | Should -Match '--no-optional-locks'
        $text | Should -Match '--untracked-files=all'
        $text | Should -Match '--ignore-submodules=none'
    }
}
