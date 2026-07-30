Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Describe 'dependency supply-chain policy' {
    BeforeAll {
        $ScriptsRoot = Split-Path -Parent $PSScriptRoot
        $RepositoryRoot = Split-Path -Parent $ScriptsRoot
        $Installer = Join-Path $ScriptsRoot 'install-cargo-deny.ps1'
        $Validator = Join-Path $ScriptsRoot 'verify-dependency-policy.ps1'
        $Tooling = Join-Path $ScriptsRoot 'release-tooling.ps1'
        $Policy = Join-Path $RepositoryRoot 'deny.toml'
        $Git = (Get-Command git.exe -CommandType Application -ErrorAction Stop |
                Select-Object -First 1).Source
    }

    It 'pins the official Windows cargo-deny binary and its reviewed digest' {
        Test-Path -LiteralPath $Installer | Should -BeTrue
        $text = Get-Content -LiteralPath $Installer -Raw
        $text | Should -Match '0\.20\.2'
        $text | Should -Match 'cargo-deny-0\.20\.2-x86_64-pc-windows-msvc\.tar\.gz'
        $text | Should -Match '975a22143262fd27476d19ee00c7af67978426e40e1dee94eed6bbade1cf87dc'
        $text | Should -Match 'C:\\Windows\\System32\\tar\.exe'
        $text | Should -Match '8388608'
        $text | Should -Not -Match 'Invoke-Expression|iex\b|Start-Process'
        { [void][scriptblock]::Create($text) } | Should -Not -Throw
    }

    It 'restricts the complete MSVC dependency graph to reviewed licenses and sources' {
        Test-Path -LiteralPath $Policy | Should -BeTrue
        $text = Get-Content -LiteralPath $Policy -Raw
        $text | Should -Match 'triple\s*=\s*"x86_64-pc-windows-msvc"'
        $text | Should -Match 'all-features\s*=\s*true'
        $text | Should -Match 'unknown-registry\s*=\s*"deny"'
        $text | Should -Match 'unknown-git\s*=\s*"deny"'
        $text | Should -Match 'allow-registry\s*=\s*\["https://github.com/rust-lang/crates.io-index"\]'
        $text | Should -Match 'allow-git\s*=\s*\[\]'
        $text | Should -Match 'unmaintained\s*=\s*"workspace"'
        $text | Should -Not -Match '(?m)^\s*ignore\s*=\s*\[[^\]]*[A-Z]+-\d'
        $text | Should -Not -Match '"Apache-2\.0 WITH LLVM-exception"|"BSD-1-Clause"|"NCSA"'
    }

    It 'runs exactly advisories licenses and sources and emits a bounded receipt' {
        Test-Path -LiteralPath $Validator | Should -BeTrue
        $text = Get-Content -LiteralPath $Validator -Raw
        $text | Should -Match '"advisories",\s*"licenses",\s*"sources"'
        $text | Should -Match 'dependency-policy\.json'
        $text | Should -Match 'policySha256'
        $text | Should -Match 'lockSha256'
        $text | Should -Match 'toolSha256'
        $text | Should -Match 'dirty'
        $text | Should -Not -Match '(?m)^\s+(arguments|output|repositoryRoot)\s*='
        { [void][scriptblock]::Create($text) } | Should -Not -Throw
    }

    It 'removes read-only task scratch without crossing its declared root' {
        Test-Path -LiteralPath $Tooling | Should -BeTrue
        . $Tooling
        $root = Join-Path $TestDrive 'owned-root'
        $scratch = Join-Path $root 'scratch'
        $file = Join-Path $scratch '.git\objects\pack\pack.idx'
        New-Item -ItemType Directory -Path (Split-Path -Parent $file) -Force | Out-Null
        Set-Content -LiteralPath $file -Value 'fixture' -Encoding utf8NoBOM
        (Get-Item -LiteralPath $file).IsReadOnly = $true

        Remove-TaskDirectory -Path $scratch -AllowedRoot $root

        Test-Path -LiteralPath $scratch | Should -BeFalse
        { Remove-TaskDirectory -Path $root -AllowedRoot $root } | Should -Throw
        { Remove-TaskDirectory -Path $TestDrive -AllowedRoot $root } | Should -Throw
    }

    It 'rejects dependency input drift between check and receipt' {
        . $Tooling
        $root = Join-Path $TestDrive 'state-repository'
        New-Item -ItemType Directory -Path $root -Force | Out-Null
        Set-Content -LiteralPath (Join-Path $root 'Cargo.lock') `
            -Value 'version = 4' -Encoding utf8NoBOM
        Set-Content -LiteralPath (Join-Path $root 'deny.toml') `
            -Value '[advisories]' -Encoding utf8NoBOM
        Set-Content -LiteralPath (Join-Path $root 'cargo-deny.exe') `
            -Value 'fixture' -Encoding utf8NoBOM
        & $Git -C $root init --quiet
        & $Git -C $root config user.email 'tokenmaster-test@example.invalid'
        & $Git -C $root config user.name 'TokenMaster Test'
        & $Git -C $root add Cargo.lock deny.toml cargo-deny.exe
        & $Git -C $root commit --quiet -m fixture

        $before = Get-DependencyPolicyState `
            -RepositoryRoot $root `
            -CargoDenyPath (Join-Path $root 'cargo-deny.exe') `
            -PolicyPath (Join-Path $root 'deny.toml') `
            -LockPath (Join-Path $root 'Cargo.lock')
        Add-Content -LiteralPath (Join-Path $root 'Cargo.lock') `
            -Value '# concurrent drift' -Encoding utf8NoBOM
        $after = Get-DependencyPolicyState `
            -RepositoryRoot $root `
            -CargoDenyPath (Join-Path $root 'cargo-deny.exe') `
            -PolicyPath (Join-Path $root 'deny.toml') `
            -LockPath (Join-Path $root 'Cargo.lock')

        { Assert-DependencyPolicyStateUnchanged -Before $before -After $after } |
            Should -Throw
    }
}
