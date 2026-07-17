Describe "TokenMaster production desktop audit" {
    BeforeAll {
        $ScriptsRoot = Split-Path -Parent $PSScriptRoot
        $RepositoryRoot = (Resolve-Path (Join-Path $ScriptsRoot "..")).Path
        $Audit = Join-Path $ScriptsRoot "audit-desktop-shell.ps1"

        function New-DesktopAuditFixture {
            param([Parameter(Mandatory = $true)][string]$Name)

            $fixture = Join-Path $TestDrive $Name
            New-Item -ItemType Directory -Path $fixture -Force | Out-Null
            Copy-Item -LiteralPath (Join-Path $RepositoryRoot "Cargo.toml") -Destination $fixture
            $crateParent = Join-Path $fixture "crates"
            New-Item -ItemType Directory -Path $crateParent -Force | Out-Null
            Copy-Item -LiteralPath (Join-Path $RepositoryRoot "crates\desktop") `
                -Destination $crateParent -Recurse
            return $fixture
        }
    }

    It "rejects probe dependencies" {
        $fixture = New-DesktopAuditFixture -Name "probe"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\Cargo.toml") `
            -Value 'tokenmaster-m0 = { path = "../probe-app" }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-PROBE-DEPENDENCY*"
    }

    It "rejects mock or seeded production data" {
        $fixture = New-DesktopAuditFixture -Name "mock"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\lib.rs") `
            -Value "fn seed_probe_models() {}"

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-MOCK-DATA*"
    }

    It "rejects the diagnostic renderer" {
        $fixture = New-DesktopAuditFixture -Name "femtovg"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\Cargo.toml") `
            -Value 'slint = { workspace = true, features = ["renderer-femtovg"] }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-FEMTOVG*"
    }

    It "rejects route-count drift" {
        $fixture = New-DesktopAuditFixture -Name "routes"
        $path = Join-Path $fixture "crates\desktop\src\presentation.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'Self::CompactWidget => "compact_widget",',
            'Self::CompactWidget => "compact_widget_extra",'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-ROUTE-COUNT*"
    }

    It "rejects direct store or runtime authority" {
        $fixture = New-DesktopAuditFixture -Name "direct-authority"
        $path = Join-Path $fixture "crates\desktop\Cargo.toml"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            '[build-dependencies]',
            "tokenmaster-store = { path = `"../store`" }`r`n`r`n[build-dependencies]"
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-DIRECT-AUTHORITY*"
    }

    It "rejects network browser shell SQL and filesystem surfaces" {
        $fixture = New-DesktopAuditFixture -Name "forbidden-authority"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\lib.rs") `
            -Value 'const PRIVATE_API: &str = "https://example.invalid";'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-FORBIDDEN-AUTHORITY*"
    }

    It "rejects a second desktop controller worker" {
        $fixture = New-DesktopAuditFixture -Name "controller-worker"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\controller.rs") `
            -Value 'fn extra_worker() { let _ = RefreshWorker::spawn('

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-CONTROLLER-WORKER*"
    }

    It "rejects query work from the Slint adapter" {
        $fixture = New-DesktopAuditFixture -Name "ui-query"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\ui.rs") `
            -Value 'fn callback_query() { let _ = DesktopController::refresh; }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-UI-QUERY*"
    }

    It "rejects a second event-loop scheduling site" {
        $fixture = New-DesktopAuditFixture -Name "bridge-event"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\bridge.rs") `
            -Value 'fn extra_event() { let _ = slint::invoke_from_event_loop('

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-BRIDGE-EVENT*"
    }

    It "rejects a bridge polling thread or timer" {
        $fixture = New-DesktopAuditFixture -Name "bridge-polling"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\bridge.rs") `
            -Value 'fn polling() { std::thread::spawn(|| {}); }'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-BRIDGE-POLLING*"
    }

    It "rejects a strong Slint window in the bridge" {
        $fixture = New-DesktopAuditFixture -Name "bridge-strong-window"
        $path = Join-Path $fixture "crates\desktop\src\bridge.rs"
        $text = [System.IO.File]::ReadAllText($path).Replace(
            'window: slint::Weak<MainWindow>',
            'window: MainWindow'
        )
        [System.IO.File]::WriteAllText($path, $text)

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-BRIDGE-WEAK*"
    }

    It "rejects a second retained product snapshot slot" {
        $fixture = New-DesktopAuditFixture -Name "bridge-second-slot"
        Add-Content -LiteralPath (Join-Path $fixture "crates\desktop\src\bridge.rs") `
            -Value 'type ExtraSlot = Arc<Mutex<Option<Arc<ProductSnapshot>>>>;'

        { & $Audit -RepositoryRoot $fixture -SourceOnly } |
            Should -Throw "*TM-DESKTOP-CONTROLLER-SLOT*"
    }
}
