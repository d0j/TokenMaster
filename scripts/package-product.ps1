param(
    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

. (Join-Path $PSScriptRoot "product-package-lib.ps1")

function Invoke-Checked {
    param(
        [Parameter(Mandatory)]
        [scriptblock]$Command,

        [Parameter(Mandatory)]
        [string]$Failure
    )

    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw $Failure
    }
}

function Get-DependencyLicenseFiles {
    param(
        [Parameter(Mandatory)]
        [string]$ManifestPath
    )

    $PackageRoot = Split-Path -Parent $ManifestPath
    $Files = [Collections.Generic.List[string]]::new()
    foreach ($File in Get-ChildItem -LiteralPath $PackageRoot -File -Force) {
        if ($File.Name -match '^(?i:LICENSE|COPYING|NOTICE|UNLICENSE)') {
            $Files.Add($File.FullName)
        }
    }
    $LicenseDirectory = Join-Path $PackageRoot "LICENSES"
    if (Test-Path -LiteralPath $LicenseDirectory -PathType Container) {
        foreach ($File in Get-ChildItem -LiteralPath $LicenseDirectory -Recurse -File -Force) {
            $Files.Add($File.FullName)
        }
    }
    $Files.Sort([StringComparer]::Ordinal)
    return $Files.ToArray()
}

$Repository = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $RepositoryRoot).Path)
$TemporaryRoot = $null
Push-Location $Repository
try {
    $Dirty = & git status --porcelain
    if ($LASTEXITCODE -ne 0) {
        throw "git status failed"
    }
    if ($Dirty) {
        throw "product packaging requires one clean commit"
    }

    $Commit = (& git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $Commit -notmatch '^[0-9a-f]{40}$') {
        throw "cannot resolve package commit"
    }

    $VsWhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path -LiteralPath $VsWhere -PathType Leaf)) {
        throw "Visual Studio locator is unavailable"
    }
    $VsInstall = (& $VsWhere -latest -products Microsoft.VisualStudio.Product.BuildTools `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath).Trim()
    if ([string]::IsNullOrWhiteSpace($VsInstall)) {
        throw "complete MSVC Build Tools installation is unavailable"
    }
    $DevShell = Join-Path $VsInstall "Common7\Tools\Microsoft.VisualStudio.DevShell.dll"
    Import-Module $DevShell -Force
    Enter-VsDevShell -VsInstallPath $VsInstall -SkipAutomaticLocation `
        -DevCmdArguments "-arch=x64 -host_arch=x64" | Out-Null

    $TargetDirectory = Get-CanonicalProductTargetDirectory -RepositoryRoot $Repository
    Invoke-Checked {
        cargo +1.97.0 build -p tokenmaster-app --release --locked `
            --target x86_64-pc-windows-msvc --target-dir $TargetDirectory
    } "canonical MSVC product build failed"

    $Executable = Join-Path $TargetDirectory `
        "x86_64-pc-windows-msvc\release\TokenMaster.exe"
    & (Join-Path $PSScriptRoot "validate-msvc-product-binary.ps1") `
        -ExecutablePath $Executable
    if ($LASTEXITCODE -ne 0) {
        throw "canonical MSVC product validation failed"
    }

    $MetadataJson = & cargo +1.97.0 metadata --locked --format-version 1
    if ($LASTEXITCODE -ne 0) {
        throw "locked Cargo metadata failed"
    }
    $Metadata = $MetadataJson | ConvertFrom-Json
    $Application = @($Metadata.packages | Where-Object { $_.name -ceq "tokenmaster-app" })
    if ($Application.Count -ne 1) {
        throw "tokenmaster-app package identity is ambiguous"
    }
    $Version = [string]$Application[0].version
    if ($Version -notmatch '^\d+\.\d+\.\d+$') {
        throw "product package version is not canonical"
    }

    $DependencyKeys = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $TreeLines = & cargo +1.97.0 tree -p tokenmaster-app --target x86_64-pc-windows-msvc `
        --locked --edges "normal,build" --prefix none --format "{p}"
    if ($LASTEXITCODE -ne 0) {
        throw "locked MSVC dependency tree failed"
    }
    foreach ($Line in $TreeLines) {
        if ($Line -match '^(?<name>[A-Za-z0-9_.-]+) v(?<version>[0-9][^ ]*)') {
            [void]$DependencyKeys.Add("$($Matches.name)@$($Matches.version)")
        }
    }
    $ApplicationKey = "tokenmaster-app@$Version"
    if (-not $DependencyKeys.Contains($ApplicationKey)) {
        throw "MSVC dependency tree omitted the product application"
    }

    $Dependencies = @(
        $Metadata.packages |
            Where-Object {
                $Key = "$($_.name)@$($_.version)"
                $DependencyKeys.Contains($Key) -and $Key -cne $ApplicationKey
            } |
            Sort-Object name, version
    )
    if ($Dependencies.Count -eq 0) {
        throw "MSVC dependency inventory is empty"
    }
    foreach ($Dependency in $Dependencies) {
        if ([string]::IsNullOrWhiteSpace([string]$Dependency.license)) {
            throw "dependency license metadata is missing: $($Dependency.name) $($Dependency.version)"
        }
    }

    $Components = @(
        foreach ($Dependency in $Dependencies) {
            [ordered]@{
                type = "library"
                name = [string]$Dependency.name
                version = [string]$Dependency.version
                licenses = @(
                    [ordered]@{ expression = [string]$Dependency.license }
                )
                purl = "pkg:cargo/$($Dependency.name)@$($Dependency.version)"
            }
        }
    )
    $Sbom = [ordered]@{
        bomFormat = "CycloneDX"
        specVersion = "1.6"
        version = 1
        metadata = [ordered]@{
            component = [ordered]@{
                type = "application"
                name = "TokenMaster"
                version = $Version
            }
        }
        components = $Components
    } | ConvertTo-Json -Depth 10

    $Notices = [Text.StringBuilder]::new()
    [void]$Notices.AppendLine("TokenMaster third-party notices")
    [void]$Notices.AppendLine("Target: x86_64-pc-windows-msvc")
    [void]$Notices.AppendLine()
    foreach ($Dependency in $Dependencies) {
        [void]$Notices.AppendLine(
            "=== $($Dependency.name) $($Dependency.version) | $($Dependency.license) ==="
        )
        if (-not [string]::IsNullOrWhiteSpace([string]$Dependency.repository)) {
            [void]$Notices.AppendLine([string]$Dependency.repository)
        }
        foreach ($LicenseFile in Get-DependencyLicenseFiles `
            -ManifestPath ([string]$Dependency.manifest_path)) {
            $PackageRoot = Split-Path -Parent ([string]$Dependency.manifest_path)
            $RelativeLicense = ([IO.Path]::GetRelativePath(
                $PackageRoot,
                $LicenseFile
            )).Replace('\', '/')
            [void]$Notices.AppendLine("--- $RelativeLicense ---")
            [void]$Notices.AppendLine([IO.File]::ReadAllText($LicenseFile))
        }
        [void]$Notices.AppendLine()
    }
    [void]$Notices.AppendLine("=== Adapted upstream references ===")
    [void]$Notices.AppendLine(
        [IO.File]::ReadAllText((Join-Path $Repository "third_party\UPSTREAM.toml"))
    )
    foreach ($UpstreamLicense in @(
        "third_party\licenses\WhereMyTokens-MIT.txt",
        "third_party\licenses\ccusage-MIT.txt"
    )) {
        [void]$Notices.AppendLine("--- $($UpstreamLicense.Replace('\', '/')) ---")
        [void]$Notices.AppendLine(
            [IO.File]::ReadAllText((Join-Path $Repository $UpstreamLicense))
        )
    }

    $ExecutableHash = (Get-FileHash -LiteralPath $Executable -Algorithm SHA256).
        Hash.ToLowerInvariant()
    $BuildInfo = [ordered]@{
        schemaVersion = 1
        status = "unsigned package candidate"
        version = $Version
        commit = $Commit
        target = "x86_64-pc-windows-msvc"
        executableSha256 = $ExecutableHash
        rust = (& rustc +1.97.0 --version).Trim()
    } | ConvertTo-Json

    $TemporaryRoot = [IO.Directory]::CreateTempSubdirectory(
        "tokenmaster-product-package-"
    ).FullName
    $StageName = "TokenMaster-$Version-windows-x64"
    $Stage = Join-Path $TemporaryRoot $StageName
    [IO.Directory]::CreateDirectory($Stage) | Out-Null
    [IO.File]::Copy($Executable, (Join-Path $Stage "TokenMaster.exe"), $false)
    [IO.File]::WriteAllBytes((Join-Path $Stage "tokenmaster.portable"), [byte[]]@())
    foreach ($Name in @("README.md", "README_RU.md", "LICENSE")) {
        [IO.File]::Copy((Join-Path $Repository $Name), (Join-Path $Stage $Name), $false)
    }
    Write-Utf8NoBomFile -Path (Join-Path $Stage "BUILDINFO.json") -Content $BuildInfo
    Write-Utf8NoBomFile -Path (Join-Path $Stage "SBOM.cdx.json") -Content $Sbom
    Write-Utf8NoBomFile -Path (Join-Path $Stage "THIRD_PARTY_NOTICES.txt") `
        -Content $Notices.ToString()
    Write-ProductChecksums -StagePath $Stage
    Assert-ProductPackageStage -StagePath $Stage

    $Dist = Join-Path $Repository "dist"
    [IO.Directory]::CreateDirectory($Dist) | Out-Null
    $Zip = Join-Path $Dist "$StageName-unsigned.zip"
    New-DeterministicZip -StagePath $Stage -DestinationPath $Zip

    & (Join-Path $PSScriptRoot "validate-product-package.ps1") `
        -RepositoryRoot $Repository `
        -PackagePath $Zip
    if ($LASTEXITCODE -ne 0) {
        throw "unsigned product package validation failed"
    }

    $Receipt = [ordered]@{
        schemaVersion = 1
        status = "unsigned package candidate"
        version = $Version
        commit = $Commit
        executableSha256 = $ExecutableHash
        packageSha256 = (Get-FileHash -LiteralPath $Zip -Algorithm SHA256).
            Hash.ToLowerInvariant()
        packageFile = [IO.Path]::GetFileName($Zip)
    } | ConvertTo-Json
    Write-Utf8NoBomFile -Path (Join-Path $Dist "$StageName-unsigned.receipt.json") `
        -Content $Receipt
    Write-Host "product-package-pass $Zip"
}
finally {
    Pop-Location
    if ($null -ne $TemporaryRoot -and [IO.Directory]::Exists($TemporaryRoot)) {
        [IO.Directory]::Delete($TemporaryRoot, $true)
    }
}
