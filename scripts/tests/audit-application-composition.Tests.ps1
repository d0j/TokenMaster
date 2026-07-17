Describe "TokenMaster application composition audit" {
    BeforeAll {
        $ScriptsRoot = Split-Path -Parent $PSScriptRoot
        $RepositoryRoot = (Resolve-Path (Join-Path $ScriptsRoot "..")).Path
        $Audit = Join-Path $ScriptsRoot "audit-application-composition.ps1"

        function New-AppAuditFixture {
            param([Parameter(Mandatory = $true)][string]$Name)

            $fixture = Join-Path $TestDrive $Name
            New-Item -ItemType Directory -Path $fixture -Force | Out-Null
            Copy-Item -LiteralPath (Join-Path $RepositoryRoot "Cargo.toml") -Destination $fixture
            $crateParent = Join-Path $fixture "crates"
            New-Item -ItemType Directory -Path $crateParent -Force | Out-Null
            Copy-Item -LiteralPath (Join-Path $RepositoryRoot "crates\app") `
                -Destination $crateParent -Recurse
            Copy-Item -LiteralPath (Join-Path $RepositoryRoot "crates\desktop") `
                -Destination $crateParent -Recurse
            return $fixture
        }
    }

    It "rejects a second live runtime owner" {
        $fixture = New-AppAuditFixture -Name "duplicate-live"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\application.rs") `
            -Value 'fn duplicate_live() { let _ = LiveRuntime::start_notified('

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-LIVE-OWNER*"
    }

    It "rejects polling threads and timers" {
        $fixture = New-AppAuditFixture -Name "polling"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\application.rs") `
            -Value 'fn polling() { std::thread::spawn(|| {}); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-POLLING*"
    }

    It "rejects command-line or working-directory roots" {
        $fixture = New-AppAuditFixture -Name "arbitrary-root"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\data_root.rs") `
            -Value 'fn cwd_root() { let _ = std::env::current_dir(); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-ARBITRARY-ROOT*"
    }

    It "rejects portable marker drift" {
        $fixture = New-AppAuditFixture -Name "marker"
        $path = Join-Path $fixture "crates\app\src\data_root.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '"tokenmaster.portable"',
            '"portable.mode"'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-PORTABLE-MARKER*"
    }

    It "rejects a strong notifier ownership cycle" {
        $fixture = New-AppAuditFixture -Name "strong-notifier"
        $path = Join-Path $fixture "crates\app\src\application.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'Weak<Mutex<Option<ApplicationBundle>>>',
            'Arc<Mutex<Option<ApplicationBundle>>>'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-WEAK-NOTIFIER*"
    }

    It "rejects probe dependencies" {
        $fixture = New-AppAuditFixture -Name "probe"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\Cargo.toml") `
            -Value 'tokenmaster-m0 = { path = "../probe-app" }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-PROBE-DEPENDENCY*"
    }

    It "rejects shell network SQL browser and credential surfaces" {
        $fixture = New-AppAuditFixture -Name "forbidden-authority"
        Add-Content -LiteralPath (Join-Path $fixture "crates\app\src\application.rs") `
            -Value 'const PRIVATE_API: &str = "https://example.invalid";'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-FORBIDDEN-AUTHORITY*"
    }

    It "rejects a second TokenMaster binary owner" {
        $fixture = New-AppAuditFixture -Name "duplicate-binary"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\Cargo.toml") `
            -Value "`r`n[[bin]]`r`nname = `"TokenMaster`"`r`npath = `"src/lib.rs`""

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-APP-DUPLICATE-BINARY*"
    }
}
