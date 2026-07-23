Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Describe 'immutable GitHub Actions references' {
    BeforeAll {
        $ScriptsRoot = Split-Path -Parent $PSScriptRoot
        $Validator = Join-Path $ScriptsRoot 'validate-immutable-actions.ps1'

        function Invoke-Validator([string]$Workflow) {
            $root = Join-Path $TestDrive ([Guid]::NewGuid().ToString('N'))
            $directory = Join-Path $root '.github\workflows'
            New-Item -ItemType Directory -Path $directory -Force | Out-Null
            Set-Content -LiteralPath (Join-Path $directory 'test.yml') `
                -Value $Workflow -Encoding utf8NoBOM
            $output = & $Validator -RepositoryRoot $root
            [pscustomobject]@{ ExitCode = $LASTEXITCODE; Output = ($output -join "`n") }
        }
    }

    It 'accepts full commit pins and repository-local actions' {
        $result = Invoke-Validator @'
jobs:
  verify:
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7
      - uses: ./.github/actions/local
'@
        $result.ExitCode | Should -Be 0
        $result.Output | Should -Be 'immutable-actions-pass'
    }

    It 'rejects tags branches expressions and abbreviated hashes' -TestCases @(
        @{ Reference = 'actions/checkout@v7' }
        @{ Reference = 'actions/checkout@main' }
        @{ Reference = 'actions/checkout@3d3c42e' }
        @{ Reference = 'actions/checkout@${{ github.ref }}' }
    ) {
        param($Reference)
        (Invoke-Validator "steps:`n  - uses: $Reference").ExitCode | Should -Be 1
    }

    It 'pins every current remote action to an exact reviewed commit' {
        $text = Get-Content -LiteralPath (
            Join-Path (Split-Path -Parent $ScriptsRoot) `
                '.github\workflows\tokenmaster-m0-windows.yml'
        ) -Raw
        $text | Should -Match `
            'actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7'
        $text | Should -Match `
            'actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a # v7'
        $text | Should -Not -Match 'uses:\s+[^@\s]+@v\d+'
    }
}
