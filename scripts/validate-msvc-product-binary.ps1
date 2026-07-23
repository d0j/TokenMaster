param(
    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path,
    [string]$ExecutablePath = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

if ([string]::IsNullOrWhiteSpace($ExecutablePath)) {
    $ExecutablePath = Join-Path $RepositoryRoot "target\x86_64-pc-windows-msvc\release\TokenMaster.exe"
}
$ExecutablePath = [IO.Path]::GetFullPath($ExecutablePath)
$ExpectedExecutablePath = [IO.Path]::GetFullPath(
    (Join-Path $RepositoryRoot "target\x86_64-pc-windows-msvc\release\TokenMaster.exe")
)
if (-not $ExecutablePath.Equals($ExpectedExecutablePath, [StringComparison]::OrdinalIgnoreCase)) {
    throw "MSVC product executable is outside the canonical Cargo target"
}
if (-not (Test-Path -LiteralPath $ExecutablePath -PathType Leaf)) {
    throw "MSVC product executable is missing"
}

$VsWhere = "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path -LiteralPath $VsWhere -PathType Leaf)) {
    throw "Visual Studio discovery is unavailable"
}
$InstallationOutput = @(& $VsWhere -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath)
$VsWhereExitCode = $LASTEXITCODE
$InstallationPath = ($InstallationOutput | Select-Object -First 1).Trim()
if ($VsWhereExitCode -ne 0 -or [string]::IsNullOrWhiteSpace($InstallationPath)) {
    throw "MSVC build tools are unavailable"
}
$Dumpbin = Get-ChildItem (Join-Path $InstallationPath "VC\Tools\MSVC") -Recurse -Filter "dumpbin.exe" -File |
    Where-Object { $_.FullName -match "\\bin\\Hostx64\\x64\\dumpbin\.exe$" } |
    Sort-Object FullName -Descending |
    Select-Object -First 1 -ExpandProperty FullName
if ([string]::IsNullOrWhiteSpace($Dumpbin)) {
    throw "MSVC binary inspector is unavailable"
}

$Headers = @(& $Dumpbin /headers $ExecutablePath)
if ($LASTEXITCODE -ne 0) {
    throw "MSVC product header inspection failed"
}
if (-not ($Headers -match "machine \(x64\)")) {
    throw "MSVC product executable is not x64"
}
if (-not ($Headers -match "subsystem \(Windows GUI\)")) {
    throw "MSVC product executable is not a Windows GUI application"
}

$Dependents = @(& $Dumpbin /dependents $ExecutablePath)
if ($LASTEXITCODE -ne 0) {
    throw "MSVC product dependency inspection failed"
}
if ($Dependents -match "(?i)\b(?:(?:VCRUNTIME|MSVCP|api-ms-win-crt-)[A-Za-z0-9_.-]*|msvcrt|ucrtbase)\.dll\b") {
    throw "MSVC product executable requires a dynamic Visual C++ runtime"
}

Write-Output "msvc-binary-pass"
