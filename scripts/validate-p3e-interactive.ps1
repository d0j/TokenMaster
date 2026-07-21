param(
    [Parameter(Mandatory = $true)]
    [string]$RepositoryRoot,
    [Parameter(Mandatory = $true)]
    [string]$ReceiptPath,
    [Parameter(Mandatory = $true)]
    [string]$ExecutablePath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Write-Failure([string]$Code) {
    [ordered]@{ result = 'fail'; code = $Code } | ConvertTo-Json -Compress
    exit 1
}

function Require-ExactProperties([object]$Value, [string[]]$Names) {
    if ($Value -isnot [pscustomobject]) { throw 'TM-P3E-TYPE' }
    $actual = @($Value.PSObject.Properties.Name | Sort-Object)
    $expected = @($Names | Sort-Object)
    if ($actual.Count -ne $expected.Count -or (Compare-Object $actual $expected)) { throw 'TM-P3E-SCHEMA' }
}

function Require-String([object]$Value) {
    if ($Value -isnot [string]) { throw 'TM-P3E-TYPE' }
}

function Require-Boolean([object]$Value) {
    if ($Value -isnot [bool]) { throw 'TM-P3E-TYPE' }
}

function Require-Number([object]$Value) {
    if ($Value -isnot [byte] -and $Value -isnot [int16] -and $Value -isnot [int32] -and $Value -isnot [int64] -and $Value -isnot [single] -and $Value -isnot [double] -and $Value -isnot [decimal]) { throw 'TM-P3E-TYPE' }
    if ([double]::IsNaN([double]$Value) -or [double]::IsInfinity([double]$Value)) { throw 'TM-P3E-TYPE' }
}

function Require-Integer([object]$Value) {
    Require-Number $Value
    if ([math]::Floor([double]$Value) -ne [double]$Value) { throw 'TM-P3E-TYPE' }
}

function Assert-NoDuplicateJsonProperties([System.Text.Json.JsonElement]$Element) {
    if ($Element.ValueKind -eq [System.Text.Json.JsonValueKind]::Object) {
        $names = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        foreach ($property in $Element.EnumerateObject()) {
            if (-not $names.Add($property.Name)) { throw 'TM-P3E-JSON' }
            Assert-NoDuplicateJsonProperties $property.Value
        }
    }
    elseif ($Element.ValueKind -eq [System.Text.Json.JsonValueKind]::Array) {
        foreach ($item in $Element.EnumerateArray()) { Assert-NoDuplicateJsonProperties $item }
    }
}

function Invoke-IsolatedGit([string[]]$Arguments) {
    $environment = [Environment]::GetEnvironmentVariables('Process')
    $names = @(
        @('GIT_DIR', 'GIT_WORK_TREE', 'GIT_INDEX_FILE', 'GIT_OBJECT_DIRECTORY', 'GIT_ALTERNATE_OBJECT_DIRECTORIES', 'GIT_COMMON_DIR', 'GIT_CONFIG_GLOBAL', 'GIT_CONFIG_SYSTEM', 'GIT_CONFIG_NOSYSTEM')
        @($environment.Keys | Where-Object { [string]$_ -match '^GIT_CONFIG_(?:COUNT|KEY_[0-9]+|VALUE_[0-9]+)$' })
    ) | Select-Object -Unique
    $prior = @{}
    try {
        foreach ($name in $names) {
            $prior[$name] = [pscustomobject]@{
                Exists = $environment.Contains($name)
                Value = [Environment]::GetEnvironmentVariable($name, 'Process')
            }
            Remove-Item -LiteralPath "Env:$name" -ErrorAction SilentlyContinue
        }
        $env:GIT_CONFIG_NOSYSTEM = '1'
        $env:GIT_CONFIG_GLOBAL = 'NUL'
        $output = @(& git.exe --no-optional-locks @Arguments 2>$null)
        [pscustomobject]@{ ExitCode = $LASTEXITCODE; Output = $output }
    }
    finally {
        foreach ($name in $names) {
            if ($prior[$name].Exists) {
                [Environment]::SetEnvironmentVariable($name, $prior[$name].Value, 'Process')
            }
            else {
                Remove-Item -LiteralPath "Env:$name" -ErrorAction SilentlyContinue
            }
        }
    }
}

try {
    if (-not (Test-Path -LiteralPath $RepositoryRoot -PathType Container) -or -not (Test-Path -LiteralPath $ReceiptPath -PathType Leaf) -or -not (Test-Path -LiteralPath $ExecutablePath -PathType Leaf)) { throw 'TM-P3E-INPUT' }
    if ((Get-Item -LiteralPath $ReceiptPath).Length -gt 32768) { throw 'TM-P3E-JSON' }
    $json = [IO.File]::ReadAllText($ReceiptPath)
    try {
        $document = [System.Text.Json.JsonDocument]::Parse($json)
        try { Assert-NoDuplicateJsonProperties $document.RootElement } finally { $document.Dispose() }
        $receipt = $json | ConvertFrom-Json -Depth 8
    }
    catch { throw 'TM-P3E-JSON' }

    Require-ExactProperties $receipt @('schema', 'result', 'commit', 'dirty', 'executableKind', 'executableSha256', 'disposableHost', 'rollback', 'scenarios', 'resources')
    foreach ($name in @('schema', 'result', 'commit', 'executableKind', 'executableSha256')) { Require-String $receipt.$name }
    foreach ($name in @('dirty', 'disposableHost')) { Require-Boolean $receipt.$name }
    if ($receipt.schema -cne 'tokenmaster.p3e.interactive.v1' -or $receipt.result -cne 'pass' -or $receipt.executableKind -cne 'packaged-production' -or $receipt.dirty -ne $false -or $receipt.disposableHost -ne $true) { throw 'TM-P3E-VALUE' }
    if ($receipt.commit -notmatch '^[0-9a-f]{40}$' -or $receipt.executableSha256 -notmatch '^[0-9a-f]{64}$') { throw 'TM-P3E-IDENTITY' }

    Require-ExactProperties $receipt.rollback @('registryPreStateRestored', 'processesStopped')
    foreach ($name in @('registryPreStateRestored', 'processesStopped')) { Require-Boolean $receipt.rollback.$name }
    if ($receipt.rollback.registryPreStateRestored -ne $true -or $receipt.rollback.processesStopped -ne $true) { throw 'TM-P3E-ROLLBACK' }

    if ($receipt.scenarios -isnot [object[]]) { throw 'TM-P3E-TYPE' }
    $requiredScenarios = @('tray_show_hide_quit', 'explorer_restart', 'secondary_activation', 'hotkey_registered', 'hotkey_conflict', 'startup_enable_readback_signin_disable', 'startup_relocation_repair_remove', 'startup_access_denied', 'lock_unlock', 'sleep_resume', 'rapid_show_hide_mode')
    if ($receipt.scenarios.Count -ne $requiredScenarios.Count) { throw 'TM-P3E-SCENARIOS' }
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($scenario in $receipt.scenarios) {
        Require-ExactProperties $scenario @('name', 'result')
        Require-String $scenario.name
        Require-String $scenario.result
        if ($scenario.result -cne 'pass' -or -not $seen.Add($scenario.name)) { throw 'TM-P3E-SCENARIOS' }
    }
    $actualScenarioNames = @($seen | Sort-Object)
    $scenarioDifference = @(Compare-Object -ReferenceObject ($requiredScenarios | Sort-Object) -DifferenceObject $actualScenarioNames)
    if ($scenarioDifference.Count -ne 0) { throw 'TM-P3E-SCENARIOS' }

    Require-ExactProperties $receipt.resources @('warmupCycles', 'measuredCycles', 'privateGrowthMiB', 'handleDelta', 'threadDelta', 'userObjectDelta', 'gdiObjectDelta')
    foreach ($name in @('warmupCycles', 'measuredCycles')) { Require-Integer $receipt.resources.$name }
    Require-Number $receipt.resources.privateGrowthMiB
    foreach ($name in @('handleDelta', 'threadDelta', 'userObjectDelta', 'gdiObjectDelta')) { Require-Integer $receipt.resources.$name }
    if ($receipt.resources.warmupCycles -lt 8 -or $receipt.resources.measuredCycles -lt 64 -or $receipt.resources.privateGrowthMiB -gt 8.0 -or $receipt.resources.handleDelta -gt 0 -or $receipt.resources.threadDelta -gt 0 -or $receipt.resources.userObjectDelta -gt 0 -or $receipt.resources.gdiObjectDelta -gt 0) { throw 'TM-P3E-RESOURCES' }

    $topLevelResult = Invoke-IsolatedGit @('-C', $RepositoryRoot, 'rev-parse', '--show-toplevel')
    $topLevel = if ($topLevelResult.Output.Count -eq 1) { [string]$topLevelResult.Output[0] } else { '' }
    $expectedRoot = (Resolve-Path -LiteralPath $RepositoryRoot).Path.TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
    $actualRoot = if ($topLevel) { (Resolve-Path -LiteralPath $topLevel).Path.TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar) } else { '' }
    if ($topLevelResult.ExitCode -ne 0 -or -not [string]::Equals($actualRoot, $expectedRoot, [StringComparison]::OrdinalIgnoreCase)) { throw 'TM-P3E-WORKTREE' }
    $status = Invoke-IsolatedGit @('-C', $RepositoryRoot, 'status', '--porcelain', '--untracked-files=all', '--ignore-submodules=none')
    if ($status.ExitCode -ne 0 -or $status.Output.Count -ne 0) { throw 'TM-P3E-WORKTREE' }
    $headResult = Invoke-IsolatedGit @('-C', $RepositoryRoot, 'rev-parse', 'HEAD')
    $head = if ($headResult.Output.Count -eq 1) { ([string]$headResult.Output[0]).Trim() } else { '' }
    if ($headResult.ExitCode -ne 0 -or $head -notmatch '^[0-9a-f]{40}$' -or $receipt.commit -cne $head) { throw 'TM-P3E-IDENTITY' }
    if ([IO.Path]::GetFileName($ExecutablePath) -cne 'tokenmaster.exe') { throw 'TM-P3E-IDENTITY' }
    $sha256 = (Get-FileHash -LiteralPath $ExecutablePath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($receipt.executableSha256 -cne $sha256) { throw 'TM-P3E-IDENTITY' }

    [ordered]@{ result = 'preflight-pass'; schema = 'tokenmaster.p3e.interactive.v1' } | ConvertTo-Json -Compress
    exit 0
}
catch {
    $code = if ($_.Exception.Message -match '^TM-P3E-[A-Z]+$') { $_.Exception.Message } else { 'TM-P3E-INVALID' }
    Write-Failure $code
}
