Describe "TokenMaster product package contracts" {
    BeforeAll {
        $ScriptsRoot = Split-Path -Parent $PSScriptRoot
        $LibraryPath = Join-Path $ScriptsRoot "product-package-lib.ps1"
        . $LibraryPath
    }

    It "creates byte-identical ZIP files from identical staged content" {
        $FixtureRoot = Join-Path $TestDrive "fixture"
        $Stage = Join-Path $FixtureRoot "TokenMaster-0.1.0-windows-x64"
        New-Item -ItemType Directory -Path $Stage -Force | Out-Null
        Set-Content -LiteralPath (Join-Path $Stage "TokenMaster.exe") -Value "binary" -NoNewline
        Set-Content -LiteralPath (Join-Path $Stage "README.md") -Value "readme" -NoNewline

        $FirstZip = Join-Path $FixtureRoot "first.zip"
        $SecondZip = Join-Path $FixtureRoot "second.zip"
        New-DeterministicZip -StagePath $Stage -DestinationPath $FirstZip

        (Get-Item -LiteralPath (Join-Path $Stage "TokenMaster.exe")).LastWriteTimeUtc =
            [DateTime]::UtcNow.AddYears(-5)
        New-DeterministicZip -StagePath $Stage -DestinationPath $SecondZip

        (Get-FileHash -LiteralPath $FirstZip -Algorithm SHA256).Hash |
            Should -Be (Get-FileHash -LiteralPath $SecondZip -Algorithm SHA256).Hash

        Add-Type -AssemblyName System.IO.Compression
        $Input = [IO.File]::OpenRead($FirstZip)
        try {
            $Archive = [IO.Compression.ZipArchive]::new(
                $Input,
                [IO.Compression.ZipArchiveMode]::Read
            )
            try {
                Test-DeterministicZipTimestamp -Timestamp $Archive.Entries[0].LastWriteTime |
                    Should -BeTrue
            }
            finally {
                $Archive.Dispose()
            }
        }
        finally {
            $Input.Dispose()
        }
    }

    It "writes a sorted SHA-256 manifest without a self-referential entry" {
        $Stage = Join-Path $TestDrive "manifest-stage"
        New-Item -ItemType Directory -Path (Join-Path $Stage "licenses") -Force | Out-Null
        Set-Content -LiteralPath (Join-Path $Stage "z.txt") -Value "last" -NoNewline
        Set-Content -LiteralPath (Join-Path $Stage "licenses\a.txt") -Value "first" -NoNewline

        Write-ProductChecksums -StagePath $Stage

        $Lines = @(Get-Content -LiteralPath (Join-Path $Stage "SHA256SUMS.txt"))
        $Lines.Count | Should -Be 2
        $Lines[0] | Should -Match '^[0-9a-f]{64}  licenses/a\.txt$'
        $Lines[1] | Should -Match '^[0-9a-f]{64}  z\.txt$'
        ($Lines -join "`n") | Should -Not -Match "SHA256SUMS"
    }

    It "accepts only the closed unsigned portable package structure" {
        $Stage = Join-Path $TestDrive "TokenMaster-0.1.0-windows-x64"
        New-Item -ItemType Directory -Path $Stage -Force | Out-Null
        [IO.File]::WriteAllBytes((Join-Path $Stage "TokenMaster.exe"), [byte[]](1, 2, 3))
        [IO.File]::WriteAllBytes((Join-Path $Stage "tokenmaster.portable"), [byte[]]@())
        foreach ($Name in @("README.md", "README_RU.md", "LICENSE")) {
            Set-Content -LiteralPath (Join-Path $Stage $Name) -Value $Name -NoNewline
        }
        Set-Content -LiteralPath (Join-Path $Stage "THIRD_PARTY_NOTICES.txt") `
            -Value "dependency 1.0 | MIT | https://example.invalid/dependency" -NoNewline

        $ExecutableHash = (Get-FileHash -LiteralPath (Join-Path $Stage "TokenMaster.exe") `
            -Algorithm SHA256).Hash.ToLowerInvariant()
        [ordered]@{
            schemaVersion = 1
            status = "unsigned package candidate"
            version = "0.1.0"
            commit = "0123456789abcdef0123456789abcdef01234567"
            target = "x86_64-pc-windows-msvc"
            executableSha256 = $ExecutableHash
        } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Stage "BUILDINFO.json") `
            -Encoding utf8NoBOM
        [ordered]@{
            bomFormat = "CycloneDX"
            specVersion = "1.6"
            version = 1
            components = @(
                [ordered]@{
                    type = "library"
                    name = "dependency"
                    version = "1.0"
                    licenses = @([ordered]@{ expression = "MIT" })
                    purl = "pkg:cargo/dependency@1.0"
                }
            )
        } | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $Stage "SBOM.cdx.json") `
            -Encoding utf8NoBOM
        Write-ProductChecksums -StagePath $Stage

        { Assert-ProductPackageStage -StagePath $Stage } | Should -Not -Throw

        Set-Content -LiteralPath (Join-Path $Stage "tokenmaster.portable") `
            -Value "not empty" -NoNewline
        { Assert-ProductPackageStage -StagePath $Stage } |
            Should -Throw "*portable marker must be empty*"
    }

    It "keeps production and validation bound to the canonical clean MSVC artifact" {
        $ProducerPath = Join-Path $ScriptsRoot "package-product.ps1"
        $ValidatorPath = Join-Path $ScriptsRoot "validate-product-package.ps1"
        foreach ($Path in @($ProducerPath, $ValidatorPath)) {
            (Test-Path -LiteralPath $Path -PathType Leaf) | Should -BeTrue
            $Text = Get-Content -LiteralPath $Path -Raw
            $Text | Should -Match "Set-StrictMode -Version Latest"
            $Text | Should -Match '\$ErrorActionPreference = "Stop"'
            { [void][scriptblock]::Create($Text) } | Should -Not -Throw
        }

        $Producer = Get-Content -LiteralPath $ProducerPath -Raw
        $Producer | Should -Match 'git status --porcelain'
        $Producer | Should -Match 'x86_64-pc-windows-msvc'
        $Producer | Should -Match '--edges "normal,build"'
        $Producer | Should -Match 'validate-msvc-product-binary\.ps1'
        $Producer | Should -Match 'New-DeterministicZip'
        $Producer | Should -Match 'Assert-ProductPackageStage'
        $Producer | Should -Not -Match 'Compress-Archive|UtcNow|Get-Date'

        $Validator = Get-Content -LiteralPath $ValidatorPath -Raw
        $Validator | Should -Match 'ZipArchiveMode\]::Read'
        $Validator | Should -Match 'Assert-ProductPackageStage'
        $Validator | Should -Match 'validate-msvc-product-binary\.ps1'
        $Validator | Should -Match 'executable hash does not match canonical MSVC build'
    }
}
