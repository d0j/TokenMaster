[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$RepositoryRoot
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$manifest = Join-Path $root 'Cargo.toml'
if (-not (Test-Path -LiteralPath $manifest -PathType Leaf)) {
    throw 'repository root does not contain Cargo.toml'
}

$metadataJson = & cargo +1.97.0 metadata --locked --format-version 1 --manifest-path $manifest
if ($LASTEXITCODE -ne 0) {
    throw 'cargo metadata failed'
}
$metadata = $metadataJson | ConvertFrom-Json -Depth 100
$rootPackages = @($metadata.packages | Where-Object { $_.name -eq 'tokenmaster-codex' })
if ($rootPackages.Count -ne 1) {
    throw 'tokenmaster-codex was not resolved exactly once'
}

$packagesById = @{}
foreach ($package in $metadata.packages) {
    $packagesById[$package.id] = $package
}
$nodesById = @{}
foreach ($node in $metadata.resolve.nodes) {
    $nodesById[$node.id] = $node
}
$visited = [System.Collections.Generic.HashSet[string]]::new()
$pending = [System.Collections.Generic.Queue[string]]::new()
$pending.Enqueue($rootPackages[0].id)
while ($pending.Count -gt 0) {
    $id = $pending.Dequeue()
    if (-not $visited.Add($id)) {
        continue
    }
    foreach ($dependency in $nodesById[$id].deps) {
        $productionKinds = @($dependency.dep_kinds | Where-Object { $_.kind -ne 'dev' })
        if ($productionKinds.Count -ne 0) {
            $pending.Enqueue($dependency.pkg)
        }
    }
}

$forbiddenCrates = @(
    'reqwest', 'ureq', 'hyper', 'hyper-util', 'h2', 'curl', 'isahc', 'surf',
    'awc', 'tokio', 'async-std', 'smol', 'webbrowser', 'headless_chrome',
    'chromiumoxide', 'thirtyfour', 'fantoccini', 'cookie_store'
)
$dependencyNames = @(
    $visited | ForEach-Object { $packagesById[$_].name } | Sort-Object -Unique
)
$forbiddenDependencies = @($dependencyNames | Where-Object { $_ -in $forbiddenCrates })
if ($forbiddenDependencies.Count -ne 0) {
    throw "Codex quota dependency closure contains forbidden crates: $($forbiddenDependencies -join ', ')"
}

$codexSourceRoot = Join-Path $root 'crates\codex\src'
$librarySourceFiles = @(
    Get-ChildItem -LiteralPath $codexSourceRoot -Recurse -File |
        Where-Object {
            $_.Extension -eq '.rs' -and
            $_.FullName -notmatch '[\\/]src[\\/]bin[\\/]'
        }
)
$forbiddenSourcePattern = @(
    'https?://',
    '\bstd::net\b|\bTcp(Stream|Listener)\b|\bUdpSocket\b',
    '\b(reqwest|ureq|isahc|fantoccini|chromiumoxide|thirtyfour)\b',
    '\b(hyper|curl|surf|awc)::',
    'headless[_-]?chrome|webbrowser|cookie[_-]?store|set-cookie',
    'auth\.json|[\\/]\.codex[\\/]auth',
    '\bAuthorization\b|\bBearer\s',
    'private[_ -]?endpoint',
    'powershell(?:\.exe)?|cmd(?:\.exe)?|bash(?:\.exe)?|\bsh\s+-c\b',
    '\bTcpStream\b|\bUdpSocket\b',
    '\b(print|eprint)ln!|\bdbg!'
) -join '|'
$sourceMatches = @(
    $librarySourceFiles | Select-String -Pattern $forbiddenSourcePattern -CaseSensitive:$false
)
if ($sourceMatches.Count -ne 0) {
    throw 'Codex production library source contains forbidden network/browser/credential/shell/logging authority'
}

$transportPath = Join-Path $root 'crates\codex\src\quota\transport.rs'
$transportText = [System.IO.File]::ReadAllText($transportPath)
$requiredTransportPatterns = @(
    'Command::new\(&self\.executable\)',
    '\.args\(\["app-server", "--stdio"\]\)',
    '"refreshToken": false',
    '"account/rateLimits/updated"',
    '"remoteControl/status/changed"',
    '\.stderr\(Stdio::null\(\)\)',
    'creation_flags\(CREATE_NO_WINDOW\)'
)
foreach ($pattern in $requiredTransportPatterns) {
    if ($transportText -notmatch $pattern) {
        throw "Codex transport is missing required fixed-boundary pattern: $pattern"
    }
}
if ([regex]::Matches($transportText, 'Command::new\(').Count -ne 1) {
    throw 'Codex transport must construct exactly one fixed child command'
}
if ([regex]::Matches($transportText, '\.args\(').Count -ne 1) {
    throw 'Codex transport must set child arguments exactly once'
}
$forbiddenTransportPattern = @(
    'Command::new\(\s*["'']',
    '\.arg\(',
    '\.env\(',
    'experimentalApi',
    'OpenOptions|File::create|File::open',
    'fs::write|fs::read_to_string',
    'TcpListener|TcpStream|UdpSocket'
) -join '|'
if ($transportText -match $forbiddenTransportPattern) {
    throw 'Codex transport contains mutable command, environment, persistence, experimental, or socket authority'
}

& cargo +1.97.0 build --release --locked --manifest-path $manifest -p tokenmaster-codex --lib
if ($LASTEXITCODE -ne 0) {
    throw 'release tokenmaster-codex library build failed'
}
$targetDirectory = [System.IO.Path]::GetFullPath([string]$metadata.target_directory)
$artifact = Get-ChildItem -LiteralPath $targetDirectory -Recurse -File |
    Where-Object {
        ($_.FullName -match '[\\/]release[\\/]deps[\\/]') -and
        ($_.Name -match '^(lib)?tokenmaster_codex-.*\.rlib$')
    } |
    Sort-Object LastWriteTimeUtc -Descending |
    Select-Object -First 1
if ($null -eq $artifact) {
    throw 'release tokenmaster-codex library artifact was not found'
}

$bytes = [System.IO.File]::ReadAllBytes($artifact.FullName)
$artifactText = [System.Text.Encoding]::ASCII.GetString($bytes)
$forbiddenBinaryStrings = @(
    'http://', 'https://', 'Set-Cookie', 'cookie_store', 'auth.json',
    'private endpoint', 'Authorization: Bearer', 'powershell.exe', 'cmd.exe',
    'TcpStream', 'TcpListener', 'UdpSocket', 'experimentalApi'
)
foreach ($needle in $forbiddenBinaryStrings) {
    if ($artifactText.IndexOf($needle, [System.StringComparison]::OrdinalIgnoreCase) -ge 0) {
        throw "release Codex library contains forbidden authority string: $needle"
    }
}

[ordered]@{
    result = 'pass'
    package = 'tokenmaster-codex'
    supported_command = 'app-server --stdio'
    dependency_count = $dependencyNames.Count
    forbidden_dependency_count = 0
    production_source_file_count = $librarySourceFiles.Count
    fixed_command_construction_count = 1
    release_artifact_count = 1
    forbidden_binary_string_count = 0
} | ConvertTo-Json -Compress
