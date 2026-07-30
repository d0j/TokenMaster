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
# The executable used to be linked against a static CRT, which made this a flat ban on
# any C runtime import. Skia has no prebuilt bindings for a static CRT and a source
# build needs LLVM, so the guarantee moved: the runtime now ships inside the portable
# package beside the binary. The invariant is unchanged -- nothing is assumed present
# on the target machine -- but it is enforced in two halves now. Here: the set of
# runtime imports may not grow beyond what the package carries. In
# `product-package-lib.ps1`: the closed content list requires those files to be present.
# A new import that nobody shipped therefore still fails, which is the case that matters.
# Two different things import-scan alike and only one of them has to ship.
#
# `ucrtbase.dll` and the `api-ms-win-crt-*` API sets are the Universal CRT, an operating
# system component since Windows 10 and serviced by Windows Update. They are present on
# every machine this product supports, so shipping copies would be wrong as well as
# unnecessary.
#
# `VCRUNTIME140*` and `MSVCP140*` are the Visual C++ redistributable. They are *not* part
# of Windows: a machine that has never run an application built with MSVC does not have
# them, and the acceptance criterion here is a stranger who downloads and runs. Those
# ship inside the portable package, and `product-package-lib.ps1` keeps them in its
# closed content list.
#
# The check that matters is therefore neither "no CRT imports" -- which is what a static
# CRT used to give, and Skia cannot -- nor "any CRT import is fine". It is: every import
# is either part of Windows, or is one the package carries.
$ShippedRuntime = @("VCRUNTIME140.dll", "VCRUNTIME140_1.dll", "MSVCP140.dll")
$OperatingSystemRuntime = "(?i)^(?:ucrtbase|api-ms-win-crt-[A-Za-z0-9_.-]+)\.dll$"
$RuntimePattern = "(?i)\b(?:(?:VCRUNTIME|MSVCP|api-ms-win-crt-)[A-Za-z0-9_.-]*|msvcrt|ucrtbase)\.dll\b"
foreach ($Line in $Dependents) {
    foreach ($Found in [regex]::Matches($Line, $RuntimePattern)) {
        $Import = $Found.Value
        if ($Import -match $OperatingSystemRuntime) { continue }
        if ($ShippedRuntime -notcontains $Import) {
            throw "MSVC product executable imports $Import, which is neither part of Windows nor shipped in the package"
        }
    }
}

Write-Output "msvc-binary-pass"
