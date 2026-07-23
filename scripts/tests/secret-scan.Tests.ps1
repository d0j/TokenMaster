Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Describe 'release secret scan' {
    BeforeAll {
        $ScriptsRoot = Split-Path -Parent $PSScriptRoot
        $RepositoryRoot = Split-Path -Parent $ScriptsRoot
        $Installer = Join-Path $ScriptsRoot 'install-gitleaks.ps1'
        $Validator = Join-Path $ScriptsRoot 'verify-secret-scan.ps1'
        $Config = Join-Path $RepositoryRoot '.gitleaks.toml'
        $Ignore = Join-Path $RepositoryRoot '.gitleaksignore'
    }

    It 'pins the reviewed official Windows x64 Gitleaks binary' {
        Test-Path -LiteralPath $Installer | Should -BeTrue
        $text = Get-Content -LiteralPath $Installer -Raw
        $text | Should -Match '8\.30\.1'
        $text | Should -Match 'gitleaks_8\.30\.1_windows_x64\.zip'
        $text | Should -Match 'd29144deff3a68aa93ced33dddf84b7fdc26070add4aa0f4513094c8332afc4e'
        $text | Should -Match '17157e2ee8b76fc8b1d8bee607a250e34b8a8023c8bc81822d4b5ee4d78fcb7c'
        $text | Should -Match 'LICENSE'
        $text | Should -Match 'README\.md'
        $text | Should -Match 'gitleaks\.exe'
        $text | Should -Not -Match 'Invoke-Expression|iex\b|Start-Process'
        { [void][scriptblock]::Create($text) } | Should -Not -Throw
    }

    It 'scans clean committed source history and the validated closed package' {
        Test-Path -LiteralPath $Validator | Should -BeTrue
        $text = Get-Content -LiteralPath $Validator -Raw
        $source = [regex]::Match(
            $text,
            '(?s)& \$Gitleaks git(?<body>.*?)if \(\$LASTEXITCODE'
        ).Groups['body'].Value
        $package = [regex]::Match(
            $text,
            '(?s)& \$Gitleaks dir(?<body>.*?)if \(\$LASTEXITCODE'
        ).Groups['body'].Value
        $text | Should -Match 'validate-product-package\.ps1'
        foreach ($invocation in @($source, $package)) {
            $invocation | Should -Match '--redact'
            $invocation | Should -Match '--timeout 300'
            $invocation | Should -Match '--max-target-megabytes 128'
            $invocation | Should -Match '--config \$Config'
            $invocation | Should -Match '--gitleaks-ignore-path \$Ignore'
        }
        $package | Should -Match '--max-archive-depth 1'
        $text | Should -Match 'secret-scan\.json'
        { [void][scriptblock]::Create($text) } | Should -Not -Throw
    }

    It 'binds a bounded receipt without retaining findings or local paths' {
        Test-Path -LiteralPath $Validator | Should -BeTrue
        $text = Get-Content -LiteralPath $Validator -Raw
        foreach ($field in @(
            'commit',
            'dirty',
            'toolVersion',
            'toolSha256',
            'packageSha256',
            'configSha256',
            'ignoreSha256',
            'sourceMode',
            'packageMode'
        )) {
            $text | Should -Match $field
        }
        $text | Should -Not -Match '(?m)^\s+(findings|output|repositoryRoot|packagePath)\s*='
    }

    It 'declares Apache-2.0 as the TokenMaster product license' {
        (Get-Content -LiteralPath (Join-Path $RepositoryRoot 'Cargo.toml') -Raw) |
            Should -Match '(?m)^license\s*=\s*"Apache-2\.0"\s*$'
        $license = Get-Content -LiteralPath (Join-Path $RepositoryRoot 'LICENSE') -Raw
        $license | Should -Match 'Apache License'
        $license | Should -Match 'Version 2\.0, January 2004'
        $license | Should -Match 'http://www\.apache\.org/licenses/'
    }

    It 'uses the reviewed config despite hostile ambient config variables' {
        Test-Path -LiteralPath $Config | Should -BeTrue
        Test-Path -LiteralPath $Ignore | Should -BeTrue
        (Get-Content -LiteralPath $Config -Raw) |
            Should -Match '(?m)^useDefault\s*=\s*true\s*$'
        (Get-Content -LiteralPath $Ignore -Raw) |
            Should -Not -Match '(?m)^\s*[0-9a-f]{40}:'

        $tool = (& $Installer -RepositoryRoot $RepositoryRoot |
                Select-Object -Last 1)
        $hostile = Join-Path $TestDrive 'hostile.toml'
        $target = Join-Path $TestDrive 'empty'
        Set-Content -LiteralPath $hostile -Value 'not valid TOML = [' -Encoding utf8NoBOM
        New-Item -ItemType Directory -Path $target | Out-Null
        $priorConfig = $env:GITLEAKS_CONFIG
        $priorConfigToml = $env:GITLEAKS_CONFIG_TOML
        try {
            $env:GITLEAKS_CONFIG = $hostile
            $env:GITLEAKS_CONFIG_TOML = 'not valid TOML = ['
            & $tool dir --no-banner --no-color --log-level error `
                --config $Config --gitleaks-ignore-path $Ignore $target
            $LASTEXITCODE | Should -Be 0
        }
        finally {
            if ($null -eq $priorConfig) {
                Remove-Item Env:GITLEAKS_CONFIG -ErrorAction SilentlyContinue
            } else {
                $env:GITLEAKS_CONFIG = $priorConfig
            }
            if ($null -eq $priorConfigToml) {
                Remove-Item Env:GITLEAKS_CONFIG_TOML -ErrorAction SilentlyContinue
            } else {
                $env:GITLEAKS_CONFIG_TOML = $priorConfigToml
            }
        }
    }
}
