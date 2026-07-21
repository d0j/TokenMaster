[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$RepositoryRoot,
    [switch]$SourceOnly
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$rootManifest = Join-Path $root 'Cargo.toml'
$desktopRoot = Join-Path $root 'crates\desktop'
$desktopManifest = Join-Path $desktopRoot 'Cargo.toml'
$sourceRoot = Join-Path $desktopRoot 'src'
$uiRoot = Join-Path $desktopRoot 'ui'

foreach ($required in @($rootManifest, $desktopManifest, $sourceRoot, $uiRoot)) {
    if (-not (Test-Path -LiteralPath $required)) {
        throw "TM-DESKTOP-MISSING-BOUNDARY: $([System.IO.Path]::GetFileName($required))"
    }
}

$manifestText = [System.IO.File]::ReadAllText($desktopManifest)
$devBoundary = $manifestText.IndexOf('[dev-dependencies]', [System.StringComparison]::Ordinal)
$productionManifestText = if ($devBoundary -ge 0) {
    $manifestText.Substring(0, $devBoundary)
} else {
    $manifestText
}

if ($manifestText -match '\btokenmaster-m0\b|[\\/]probe-app\b') {
    throw 'TM-DESKTOP-PROBE-DEPENDENCY: production desktop must not depend on the M0 probe'
}
if ($manifestText -match '\brenderer-femtovg\b') {
    throw 'TM-DESKTOP-FEMTOVG: production desktop must remain software-renderer only'
}
if ($productionManifestText -match '\btokenmaster-(store|provider|runtime|codex|git|platform)\b|\b(rusqlite|libsqlite3-sys|notify)\b') {
    throw 'TM-DESKTOP-DIRECT-AUTHORITY: desktop manifest contains a forbidden direct authority dependency'
}

$rustFiles = @(Get-ChildItem -LiteralPath $sourceRoot -Recurse -File -Filter '*.rs')
$uiFiles = @(Get-ChildItem -LiteralPath $uiRoot -Recurse -File -Filter '*.slint')
$productionFiles = @($rustFiles + $uiFiles)
if ($rustFiles.Count -ne 18 -or $uiFiles.Count -ne 24) {
    throw 'TM-DESKTOP-FILE-COUNT: production desktop boundary must contain eighteen Rust and twenty-four Slint files'
}
$uiText = ($uiFiles | ForEach-Object {
    [System.IO.File]::ReadAllText($_.FullName)
}) -join "`n"
$productionText = ($productionFiles | ForEach-Object {
    [System.IO.File]::ReadAllText($_.FullName)
}) -join "`n"

if ($productionText -match '\b(seed_probe_models|mock|fixture|seeded|seed)\b') {
    throw 'TM-DESKTOP-MOCK-DATA: production desktop contains mock or seeded data'
}
$forbiddenAuthorityPattern = @(
    'https?://',
    '\bstd::(fs|net|process)\b',
    '\b(Command|TcpStream|TcpListener|UdpSocket)\b',
    '\b(rusqlite|notify|reqwest|ureq|webbrowser|headless_chrome)\b',
    '\b(SELECT|INSERT|UPDATE|DELETE\s+FROM|PRAGMA)\b',
    'powershell(?:\.exe)?|cmd(?:\.exe)?|bash(?:\.exe)?|\bsh\s+-c\b',
    'auth\.json|[\\/]\.codex[\\/]auth|\bAuthorization\b|\bBearer\s'
) -join '|'
if ($productionText -cmatch $forbiddenAuthorityPattern) {
    throw 'TM-DESKTOP-FORBIDDEN-AUTHORITY: desktop source contains filesystem/network/process/SQL/browser/credential authority'
}

$controllerPath = Join-Path $sourceRoot 'controller.rs'
$controllerText = [System.IO.File]::ReadAllText($controllerPath)
$bridgePath = Join-Path $sourceRoot 'bridge.rs'
$bridgeText = [System.IO.File]::ReadAllText($bridgePath)
$uiRustPath = Join-Path $sourceRoot 'ui.rs'
$uiRustText = [System.IO.File]::ReadAllText($uiRustPath)
$uiRustTestBoundary = $uiRustText.IndexOf('#[cfg(test)]', [System.StringComparison]::Ordinal)
$uiRustProductionText = if ($uiRustTestBoundary -ge 0) {
    $uiRustText.Substring(0, $uiRustTestBoundary)
} else {
    $uiRustText
}
$reliableStateText = [System.IO.File]::ReadAllText((Join-Path $sourceRoot 'reliable_state.rs'))
$appRoot = Join-Path $root 'crates\app\src'
$appStatePath = Join-Path $appRoot 'state.rs'
$appOperationTestsPath = Join-Path $appRoot 'operation_tests.rs'
foreach ($required in @($appStatePath, $appOperationTestsPath)) {
    if (-not (Test-Path -LiteralPath $required)) {
        throw "TM-DESKTOP-MISSING-BOUNDARY: $([System.IO.Path]::GetFileName($required))"
    }
}
$appStateText = [System.IO.File]::ReadAllText($appStatePath)
$appOperationTestsText = [System.IO.File]::ReadAllText($appOperationTestsPath)

function ConvertTo-ExecutableText {
    param([Parameter(Mandatory = $true)][string]$Text, [switch]$PreserveLiteralText)

    $output = New-Object System.Text.StringBuilder $Text.Length
    $index = 0; $blockDepth = 0
    while ($index -lt $Text.Length) {
        $character = $Text[$index]
        $next = if ($index + 1 -lt $Text.Length) { $Text[$index + 1] } else { [char]0 }
        if ($blockDepth -gt 0) {
            if ($character -eq '/' -and $next -eq '*') { [void]$output.Append('  '); $blockDepth++; $index += 2; continue }
            if ($character -eq '*' -and $next -eq '/') { [void]$output.Append('  '); $blockDepth--; $index += 2; continue }
            [void]$output.Append($(if ($character -eq "`n" -or $character -eq "`r") { $character } else { ' ' })); $index++; continue
        }
        if ($character -eq '/' -and $next -eq '/') {
            [void]$output.Append('  '); $index += 2
            while ($index -lt $Text.Length -and $Text[$index] -ne "`n") { [void]$output.Append(' '); $index++ }
            continue
        }
        if ($character -eq '/' -and $next -eq '*') { [void]$output.Append('  '); $blockDepth = 1; $index += 2; continue }
        $rawStart = if ($character -eq 'r') { $index } elseif ($character -eq 'b' -and $next -eq 'r') { $index + 1 } else { -1 }
        if ($rawStart -ge 0) {
            $hashIndex = $rawStart + 1
            while ($hashIndex -lt $Text.Length -and $Text[$hashIndex] -eq '#') { $hashIndex++ }
            if ($hashIndex -lt $Text.Length -and $Text[$hashIndex] -eq '"') {
                $hashCount = $hashIndex - $rawStart - 1
                while ($index -le $hashIndex) {
                    [void]$output.Append($(if ($PreserveLiteralText) { $Text[$index] } elseif ($Text[$index] -eq "`n" -or $Text[$index] -eq "`r") { $Text[$index] } else { ' ' }))
                    $index++
                }
                while ($index -lt $Text.Length) {
                    $literalCharacter = $Text[$index]
                    [void]$output.Append($(if ($PreserveLiteralText) { $literalCharacter } elseif ($literalCharacter -eq "`n" -or $literalCharacter -eq "`r") { $literalCharacter } else { ' ' }))
                    $index++
                    if ($literalCharacter -eq '"') {
                        $closing = $true
                        for ($hash = 0; $hash -lt $hashCount; $hash++) { if ($index + $hash -ge $Text.Length -or $Text[$index + $hash] -ne '#') { $closing = $false; break } }
                        if ($closing) { for ($hash = 0; $hash -lt $hashCount; $hash++) { [void]$output.Append($(if ($PreserveLiteralText) { $Text[$index] } else { ' ' })); $index++ }; break }
                    }
                }
                continue
            }
        }
        $stringStart = $character -eq '"' -or ($character -eq 'b' -and $next -eq '"')
        $byteCharacter = $character -eq 'b' -and $next -eq "'" -and $index + 3 -lt $Text.Length -and $Text[$index + 3] -eq "'"
        $characterLiteral = $character -eq "'" -and (($index + 2 -lt $Text.Length -and $Text[$index + 2] -eq "'") -or ($index + 3 -lt $Text.Length -and $Text[$index + 1] -eq '\' -and $Text[$index + 3] -eq "'"))
        if ($stringStart -or $byteCharacter -or $characterLiteral) {
            $quote = if ($stringStart) { '"' } else { "'" }
            $literalOpeningQuote = if (($stringStart -or $byteCharacter) -and $character -eq 'b') { $index + 1 } else { $index }
            do {
                $literalCharacter = $Text[$index]
                [void]$output.Append($(if ($PreserveLiteralText) { $literalCharacter } elseif ($literalCharacter -eq "`n" -or $literalCharacter -eq "`r") { $literalCharacter } else { ' ' }))
                if ($literalCharacter -eq '\' -and $index + 1 -lt $Text.Length) { $index++; [void]$output.Append($(if ($PreserveLiteralText) { $Text[$index] } else { ' ' })) }
                elseif ($literalCharacter -eq $quote -and $index -gt $literalOpeningQuote) { $index++; break }
                $index++
            } while ($index -lt $Text.Length)
            continue
        }
        [void]$output.Append($character); $index++
    }
    return $output.ToString()
}

function Get-ExecutableBracedText {
    param([Parameter(Mandatory = $true)][string]$Text, [Parameter(Mandatory = $true)][string]$Pattern, [Parameter(Mandatory = $true)][string]$FailureCode, [switch]$PreserveLiteralText)
    $executable = ConvertTo-ExecutableText -Text $Text
    $match = [regex]::Match($executable, $Pattern)
    if (-not $match.Success) { throw "${FailureCode}: missing executable structure" }
    $open = $executable.IndexOf('{', $match.Index)
    if ($open -lt 0) { throw "${FailureCode}: missing executable structure body" }
    $depth = 0
    for ($index = $open; $index -lt $executable.Length; $index++) {
        if ($executable[$index] -eq '{') { $depth++ }
        if ($executable[$index] -eq '}') { $depth--; if ($depth -eq 0) { return ConvertTo-ExecutableText -Text $Text.Substring($match.Index, $index - $match.Index + 1) -PreserveLiteralText:$PreserveLiteralText } }
    }
    throw "${FailureCode}: unclosed executable structure"
}

function Get-RustFunctionText {
    param([Parameter(Mandatory = $true)][string]$Text, [Parameter(Mandatory = $true)][string]$Name, [switch]$PreserveLiteralText)
    return Get-ExecutableBracedText -Text $Text -Pattern "(?m)^\s*(?:pub(?:\s*\([^)]*\))?\s+)?(?:const\s+)?fn\s+$Name\s*\(" -FailureCode 'TM-DESKTOP-DENSITY-WIRING' -PreserveLiteralText:$PreserveLiteralText
}

function Normalize-ExecutableStructure { param([Parameter(Mandatory = $true)][string]$Text); return [regex]::Replace($Text, '\s+', '') }

$presentationStylePath = Join-Path $sourceRoot 'presentation_style.rs'
$presentationStyleText = [System.IO.File]::ReadAllText($presentationStylePath)
$presentationStyleContractPath = Join-Path $desktopRoot 'tests\presentation_style_contract.rs'
$presentationStyleContractText = [System.IO.File]::ReadAllText($presentationStyleContractPath)
$mainUiTextForDensity = [System.IO.File]::ReadAllText((Join-Path $uiRoot 'main.slint'))
$tokensText = [System.IO.File]::ReadAllText((Join-Path $uiRoot 'tokens.slint'))
$densityWireText = Get-RustFunctionText -Text $uiRustProductionText -Name 'wire_presentation_density'
$densityApplyText = Get-RustFunctionText -Text $uiRustProductionText -Name 'apply_presentation_style'
$stableKeyText = Get-RustFunctionText -Text $presentationStyleText -Name 'stable_key'
$slintIndexText = Get-RustFunctionText -Text $presentationStyleText -Name 'slint_index'
$fromSlintIndexText = Get-RustFunctionText -Text $presentationStyleText -Name 'from_slint_index'
$checkedSuccessorText = Get-RustFunctionText -Text $presentationStyleText -Name 'checked_successor'
$selectDensityText = Get-RustFunctionText -Text $presentationStyleText -Name 'select_density_index'
$selectDensityIfAdmittedText = Get-RustFunctionText -Text $presentationStyleText -Name 'select_density_index_if_admitted'
$densityEnumText = Get-ExecutableBracedText -Text $presentationStyleText -Pattern '(?m)^\s*pub\s+enum\s+DesktopDensity\s*\{' -FailureCode 'TM-DESKTOP-DENSITY-CONTRACT'
$densityVariantMatches = [regex]::Matches($densityEnumText, '(?m)^\s*(?<variant>[A-Za-z][A-Za-z0-9_]*)\s*,\s*$')
$densityVariantCount = $densityVariantMatches.Count
$stableKeyText = Get-RustFunctionText -Text $presentationStyleText -Name 'stable_key' -PreserveLiteralText
$stableKeyArmCount = [regex]::Matches($stableKeyText, '(?m)^\s*Self::[A-Za-z][A-Za-z0-9_]*\s*=>\s*"[^"]+",\s*$').Count
$slintIndexArmCount = [regex]::Matches($slintIndexText, '(?m)^\s*Self::[A-Za-z][A-Za-z0-9_]*\s*=>\s*\d+,\s*$').Count
$fromSlintIndexArmCount = [regex]::Matches($fromSlintIndexText, '(?m)^\s*\d+\s*=>\s*Some\(Self::[A-Za-z][A-Za-z0-9_]*\),\s*$').Count
$densityPairs = @(
    @{ Variant = 'Comfortable'; Key = 'comfortable'; Index = 0 },
    @{ Variant = 'Compact'; Key = 'compact'; Index = 1 },
    @{ Variant = 'UltraCompact'; Key = 'ultra_compact'; Index = 2 }
)
if ($densityVariantCount -ne 3 -or $stableKeyArmCount -ne 3 -or
    $slintIndexArmCount -ne 3 -or $fromSlintIndexArmCount -ne 3) {
    throw 'TM-DESKTOP-DENSITY-CONTRACT: density must retain exactly three variants and three arms per mapping'
}
foreach ($density in $densityPairs) {
    $keyPattern = "Self::$($density.Variant) => `"$($density.Key)`","
    $indexPattern = "Self::$($density.Variant) => $($density.Index),"
    $fromIndexPattern = "$($density.Index) => Some(Self::$($density.Variant)),"
    if ([regex]::Matches($stableKeyText, [regex]::Escape($keyPattern)).Count -ne 1 -or
        [regex]::Matches($slintIndexText, [regex]::Escape($indexPattern)).Count -ne 1 -or
        [regex]::Matches($fromSlintIndexText, [regex]::Escape($fromIndexPattern)).Count -ne 1) {
        throw 'TM-DESKTOP-DENSITY-CONTRACT: density keys and Slint indices must remain the exact fixed three-value mapping'
    }
}
$densityTokenTables = @(
    'out property <length> space-xs: density-id == 2 ? 2px : (density-id == 1 ? 3px : 4px);',
    'out property <length> space-sm: density-id == 2 ? 4px : (density-id == 1 ? 6px : 8px);',
    'out property <length> space: density-id == 2 ? 8px : (density-id == 1 ? 12px : 16px);',
    'out property <length> space-lg: density-id == 2 ? 12px : (density-id == 1 ? 18px : 24px);',
    'out property <length> radius-sm: density-id == 2 ? 3px : (density-id == 1 ? 4px : 5px);',
    'out property <length> radius: density-id == 2 ? 4px : (density-id == 1 ? 6px : 8px);',
    'out property <length> radius-lg: density-id == 2 ? 6px : (density-id == 1 ? 9px : 12px);'
)
$uiTokensText = Get-ExecutableBracedText -Text $tokensText -Pattern '(?m)^\s*export\s+global\s+UiTokens\s*\{' -FailureCode 'TM-DESKTOP-DENSITY-TOKENS' -PreserveLiteralText
$densityTokenDeclarations = @([regex]::Matches(
    $uiTokensText,
    '(?s)\bout\s+property\s*<\s*length\s*>\s*(?<name>[a-z][a-z-]*)\s*:\s*(?<expression>[^;]*\bdensity-id\b[^;]*);'
) | ForEach-Object {
    "out property <length> $($_.Groups['name'].Value): $($_.Groups['expression'].Value);" -replace '\s+', ' '
})
$densityTokenDeclarationCount = $densityTokenDeclarations.Count
$normalizedDensityTokenTables = @($densityTokenTables | ForEach-Object { $_ -replace '\s+', ' ' })
if ($densityTokenDeclarationCount -ne 7 -or @($normalizedDensityTokenTables | Where-Object {
        $expectedToken = $_
        @($densityTokenDeclarations | Where-Object { $_ -eq $expectedToken }).Count -ne 1
    }).Count -ne 0) {
    throw 'TM-DESKTOP-DENSITY-TOKENS: density must retain exactly seven fixed token tables including space-lg 24/18/12'
}
$presentationStyleOwnerCount = [regex]::Matches($presentationStyleText, 'pub struct DesktopPresentationStyle\s*\{').Count
$presentationStyleOwnerSlotCount = [regex]::Matches(
    $uiRustProductionText,
    'Arc::new\(Mutex::new\(initial_presentation_style\)\)'
).Count
$rootDensityBindingCount = [regex]::Matches(
    $mainUiTextForDensity,
    'in-out property <int> presentation-density-id <=> UiTokens\.density-id;'
).Count
$rootDensityCallbackCount = [regex]::Matches(
    $mainUiTextForDensity,
    'callback select-presentation-density\(int\);'
).Count
$densityWiringCallbackCount = [regex]::Matches(
    $densityWireText,
    'window\.on_select_presentation_density\(move \|index\| \{'
).Count
if ($presentationStyleOwnerCount -ne 1 -or $presentationStyleOwnerSlotCount -ne 1 -or
    $rootDensityBindingCount -ne 1 -or $rootDensityCallbackCount -ne 1 -or
    $densityWiringCallbackCount -ne 1) {
    throw 'TM-DESKTOP-DENSITY-WIRING: density must retain one owner, one root binding, and one callback'
}
$skinPath = Join-Path $sourceRoot 'skin.rs'
$skinText = [System.IO.File]::ReadAllText($skinPath)
$skinEnum = Get-ExecutableBracedText -Text $skinText -Pattern '(?m)^\s*pub\s+enum\s+DesktopSkin\s*\{' -FailureCode 'TM-DESKTOP-SKIN-CONTRACT'
$skinStableKeyText = Get-RustFunctionText -Text $skinText -Name 'stable_key' -PreserveLiteralText
$skinSlintIndexText = Get-RustFunctionText -Text $skinText -Name 'slint_index'
$skinFromSlintIndexText = Get-RustFunctionText -Text $skinText -Name 'from_slint_index'
$skinExpectedEnum = 'pubenumDesktopSkin{Refined,Graphite,Ember,}'
$skinVariantCount = [regex]::Matches($skinEnum, '(?m)^\s*[A-Za-z][A-Za-z0-9_]*\s*,\s*$').Count
$skinKeyMappingCount = [regex]::Matches($skinStableKeyText, '(?m)^\s*Self::[A-Za-z][A-Za-z0-9_]*\s*=>\s*"[^"]+",\s*$').Count
$skinIndexMappingCount = [regex]::Matches($skinSlintIndexText, '(?m)^\s*Self::[A-Za-z][A-Za-z0-9_]*\s*=>\s*\d+,\s*$').Count
$skinReverseIndexMappingCount = [regex]::Matches($skinFromSlintIndexText, '(?m)^\s*\d+\s*=>\s*Some\(Self::[A-Za-z][A-Za-z0-9_]*\),\s*$').Count
if ((Normalize-ExecutableStructure -Text $skinEnum) -ne $skinExpectedEnum -or
    $skinVariantCount -ne 3 -or $skinKeyMappingCount -ne 3 -or $skinIndexMappingCount -ne 3 -or $skinReverseIndexMappingCount -ne 3) { throw 'TM-DESKTOP-SKIN-CONTRACT: exactly three skins are admitted' }
foreach ($skin in @(@{ Variant = 'Refined'; Key = 'refined'; Index = 0 }, @{ Variant = 'Graphite'; Key = 'graphite'; Index = 1 }, @{ Variant = 'Ember'; Key = 'ember'; Index = 2 })) {
    foreach ($mapping in @("Self::$($skin.Variant) => `"$($skin.Key)`",", "Self::$($skin.Variant) => $($skin.Index),", "$($skin.Index) => Some(Self::$($skin.Variant)),")) {
        if ([regex]::Matches($skinStableKeyText, [regex]::Escape($mapping)).Count -ne 1 -and
            [regex]::Matches($skinSlintIndexText, [regex]::Escape($mapping)).Count -ne 1 -and
            [regex]::Matches($skinFromSlintIndexText, [regex]::Escape($mapping)).Count -ne 1) { throw 'TM-DESKTOP-SKIN-CONTRACT: skin keys and indices are exact' }
    }
}
$paletteRoles = @('background','surface','surface_raised','surface_subtle','border','text_primary','text_secondary','accent','accent_subtle','accent_secondary','accent_tertiary','ready','waiting','degraded','unavailable')
$tokenStruct = Get-ExecutableBracedText -Text $skinText -Pattern '(?m)^\s*pub\s+struct\s+DesktopColorTokens\s*\{' -FailureCode 'TM-DESKTOP-SKIN-PALETTE'
foreach ($role in $paletteRoles) {
    if ([regex]::Matches($tokenStruct, "(?m)^\s*$role\s*:\s*DesktopRgb,\s*$").Count -ne 1 -or [regex]::Matches($tokensText, "out\s+property\s*<color>\s+$($role -replace '_','-')\s*:\s*palette\.$($role -replace '_','-')\s*;").Count -ne 1) { throw 'TM-DESKTOP-SKIN-PALETTE: one palette owns exactly fifteen roles' }
}
$expectedPaletteRgb = [ordered]@{
    refined_tokens = @('background=11,15,23','surface=17,24,39','surface_raised=24,34,52','surface_subtle=14,22,36','border=41,53,72','text_primary=244,247,251','text_secondary=158,171,192','accent=124,212,253','accent_subtle=23,48,68','accent_secondary=167,139,250','accent_tertiary=240,171,252','ready=112,214,165','waiting=143,163,191','degraded=242,198,109','unavailable=240,139,139')
    graphite_tokens = @('background=16,18,22','surface=24,27,32','surface_raised=34,38,45','surface_subtle=20,23,28','border=52,58,68','text_primary=245,247,250','text_secondary=170,178,189','accent=120,169,255','accent_subtle=31,45,69','accent_secondary=165,180,252','accent_tertiary=216,180,254','ready=115,215,173','waiting=154,167,184','degraded=234,197,116','unavailable=238,141,147')
    ember_tokens = @('background=20,13,10','surface=32,21,17','surface_raised=46,31,25','surface_subtle=25,15,12','border=75,48,38','text_primary=255,247,237','text_secondary=205,176,157','accent=251,146,60','accent_subtle=71,36,23','accent_secondary=251,191,36','accent_tertiary=244,114,182','ready=134,212,157','waiting=189,169,158','degraded=244,200,111','unavailable=245,143,134')
}
$paletteRgbValueCount = 0
foreach ($functionName in $expectedPaletteRgb.Keys) {
    $paletteFunction = Get-RustFunctionText -Text $skinText -Name $functionName
    $rgbRoles = @([regex]::Matches($paletteFunction, '(?<role>[a-z_]+)\s*:\s*rgb\((?<rgb>\d+\s*,\s*\d+\s*,\s*\d+)\)') | ForEach-Object { "$($_.Groups['role'].Value)=$($_.Groups['rgb'].Value -replace '\s+', '')" })
    $missingPaletteValues = @()
    foreach ($expectedPaletteValue in $expectedPaletteRgb[$functionName]) {
        if (@($rgbRoles | Where-Object { $_ -eq $expectedPaletteValue }).Count -ne 1) {
            $missingPaletteValues += $expectedPaletteValue
        }
    }
    if ($rgbRoles.Count -ne 15 -or $missingPaletteValues.Count -ne 0) { throw 'TM-DESKTOP-SKIN-PALETTE: every palette role must retain its exact RGB value' }
    $paletteRgbValueCount += $rgbRoles.Count
}
if ($paletteRgbValueCount -ne 45 -or $uiText -match '(?i)\b(?:refined|graphite|ember)[-_]?(?:palette|family|theme)\b') { throw 'TM-DESKTOP-SKIN-PALETTE: exact Rust palettes and Slint aliases are required' }
$skinRootBindingCount = [regex]::Matches($mainUiTextForDensity, 'in-out property <UiPalette> presentation-palette <=> UiTokens\.palette;').Count
$skinRootCallbackCount = [regex]::Matches($mainUiTextForDensity, 'callback select-presentation-skin\(int\);').Count
$mainWindowText = Get-ExecutableBracedText -Text $mainUiTextForDensity -Pattern '(?m)^\s*export\s+component\s+MainWindow\s+(?:inherits\s+\w+\s*)?\{' -FailureCode 'TM-DESKTOP-PRESENTATION-OWNER'
$paletteSlotCount = [regex]::Matches($mainWindowText, 'in-out\s+property\s*<\s*UiPalette\s*>').Count
$paletteAliasCount = [regex]::Matches($uiTokensText, 'out\s+property\s*<\s*(?:color|brush)\s*>').Count
$settingsSkinCallbackCount = [regex]::Matches([System.IO.File]::ReadAllText((Join-Path $uiRoot 'views\settings-view.slint')), 'callback select-presentation-skin\(int\);').Count
$skinForwardBindingCount = [regex]::Matches($mainUiTextForDensity, 'select-presentation-skin\(index\) => \{ root\.select-presentation-skin\(index\); \}').Count
$skinWireText = Get-RustFunctionText -Text $uiRustProductionText -Name 'wire_presentation_skin'
$skinWiringCallbackCount = [regex]::Matches($skinWireText, 'window\.on_select_presentation_skin\(move \|index\| \{').Count
if ($presentationStyleOwnerCount -ne 1 -or $presentationStyleOwnerSlotCount -ne 1 -or $paletteSlotCount -ne 1 -or $paletteAliasCount -ne 15 -or $skinRootBindingCount -ne 1 -or $skinRootCallbackCount -ne 1 -or $settingsSkinCallbackCount -ne 1 -or $skinForwardBindingCount -ne 1 -or $skinWiringCallbackCount -ne 1 -or [regex]::Matches($uiRustProductionText, 'Arc\s*<\s*Mutex\s*<\s*DesktopPresentationStyle\s*>\s*>').Count -ne 7) { throw 'TM-DESKTOP-PRESENTATION-OWNER: exactly one complete presentation owner and palette slot are required' }
$presentationApplyText = Get-RustFunctionText -Text $uiRustProductionText -Name 'apply_presentation_style'
$paletteIndex = $presentationApplyText.IndexOf('window.set_presentation_palette(ui_palette(style.skin()));', [System.StringComparison]::Ordinal)
$metadataIndex = $presentationApplyText.IndexOf('window.set_presentation_revision', [System.StringComparison]::Ordinal)
if ($paletteIndex -lt 0 -or $metadataIndex -le $paletteIndex -or $presentationApplyText.Substring($paletteIndex, $metadataIndex - $paletteIndex) -match 'invoke_from_event_loop|run_event_loop|\.show\(|yield') { throw 'TM-DESKTOP-PRESENTATION-ORDER: palette assignment must precede metadata without a yield or show' }
$presentationAuthorityText = ConvertTo-ExecutableText -Text ($skinText + "`n" + $presentationStyleText + "`n" + $presentationApplyText + "`n" + $skinWireText)
$presentationAuthorityText = [regex]::Replace($presentationAuthorityText, 'Arc\s*<\s*Mutex\s*<\s*DesktopPresentationStyle\s*>\s*>', '')
if ($presentationAuthorityText -match '(?i)\b(?:thread|timer|delay|interval|worker|queue|vecdeque|deque|sync_channel|mpsc|channel|sender|receiver|[a-z_][a-z0-9_]*(?:cache|query)|createwindow\w*|unsafe|std\s*::\s*(?:fs|net|process)|tcpstream|sql|(?:vec|box|hashmap|once|oncelock|cell|refcell|mutex|arc)\s*(?:::)?\s*(?:<|::))\b') { throw 'TM-DESKTOP-DENSITY-NO-AUTHORITY: presentation must not gain authority' }
 $checkedSuccessorCurrent = Get-RustFunctionText -Text $presentationStyleText -Name 'checked_successor'
if ($presentationAuthorityText -notmatch 'pub struct DesktopPresentationRevision\(u64\);' -or $checkedSuccessorCurrent -notmatch 'self\.0\.checked_add\(1\)' -or $presentationAuthorityText -notmatch 'self\.revision\s*=\s*revision;') { throw 'TM-DESKTOP-DENSITY-REVISION: complete presentation revisions remain checked and assigned' }
if ([regex]::Matches($appOperationTestsText, 'fn\s+presentation_follow_up_replaces_only_the_pending_complete_payload\s*\(').Count -ne 1 -or [regex]::Matches($appOperationTestsText, 'fn\s+ten_thousand_presentation_updates_keep_one_latest_payload\s*\(').Count -ne 1) { throw 'TM-DESKTOP-DENSITY-STRESS: complete presentation proofs must remain executable' }
$presentationStressCurrent = Get-RustFunctionText -Text $appOperationTestsText -Name 'ten_thousand_presentation_updates_keep_one_latest_payload'
if ([regex]::Matches($presentationStyleContractText, 'DesktopPresentationApplyOutcome::Applied').Count -lt 2 -or $presentationStressCurrent -notmatch 'for index in 0\.\.10_000' -or $presentationStressCurrent -notmatch 'assert_eq!\(snapshot\.active_count\(\), 1\);' -or $presentationStressCurrent -notmatch 'assert_eq!\(snapshot\.pending_count\(\), 1\);' -or $presentationStressCurrent -notmatch 'assert_eq!\(receive\(&started_rx\), final_selection\);') { throw 'TM-DESKTOP-DENSITY-STRESS: complete presentation keeps semantic latest-wins proof' }
if ($uiRustProductionText -match 'select_density_index\(index\);' -or $presentationStyleText -match 'self\.selection\s*=\s*selection;\s*if\s*!admit\(selection\)') { throw 'TM-DESKTOP-DENSITY-ADMISSION: complete selection must admit before any UI mutation' }
$backupPresentationText = Get-RustFunctionText -Text $appStateText -Name 'update_backup_policy'
$reminderPresentationText = Get-RustFunctionText -Text $appStateText -Name 'update_reminder_policy'
if ($backupPresentationText -notmatch '\*current\.value\(\)\.portable\(\)\.presentation\(\)' -or $reminderPresentationText -notmatch '\*current\.value\(\)\.portable\(\)\.presentation\(\)') { throw 'TM-DESKTOP-PRESENTATION-PRESERVATION: settings mutations retain the complete presentation pair' }
if ($presentationStyleText.Length -gt 0) {
$presentationStyleExecutableText = ConvertTo-ExecutableText -Text $presentationStyleText
$presentationRevisionTypeCount = [regex]::Matches(
    $presentationStyleExecutableText,
    'pub struct DesktopPresentationRevision\(u64\);'
).Count
$expectedCheckedSuccessor = 'constfnchecked_successor(self)->Option<Self>{matchself.0.checked_add(1){Some(value)=>Some(Self(value)),None=>None,}}'
$expectedSelectDensity = 'pubfnselect_density_index(&mutself,index:i32)->DesktopPresentationApplyOutcome{letSome(density)=DesktopDensity::from_slint_index(index)else{returnDesktopPresentationApplyOutcome::Rejected;};self.select(self.selection.with_density(density),false,|_|true)}'
$expectedSelectDensityIfAdmitted = 'pubfnselect_density_index_if_admitted(&mutself,index:i32,admit:implFnOnce(DesktopPresentationSelection)->bool,)->DesktopPresentationApplyOutcome{letSome(density)=DesktopDensity::from_slint_index(index)else{returnDesktopPresentationApplyOutcome::Rejected;};self.select(self.selection.with_density(density),true,admit)}'
$selectSkinText = Get-RustFunctionText -Text $presentationStyleText -Name 'select_skin_index'
$selectSkinIfAdmittedText = Get-RustFunctionText -Text $presentationStyleText -Name 'select_skin_index_if_admitted'
$expectedSelectSkin = 'pubfnselect_skin_index(&mutself,index:i32)->DesktopPresentationApplyOutcome{letSome(skin)=DesktopSkin::from_slint_index(index)else{returnDesktopPresentationApplyOutcome::Rejected;};self.select(self.selection.with_skin(skin),false,|_|true)}'
$expectedSelectSkinIfAdmitted = 'pubfnselect_skin_index_if_admitted(&mutself,index:i32,admit:implFnOnce(DesktopPresentationSelection)->bool,)->DesktopPresentationApplyOutcome{letSome(skin)=DesktopSkin::from_slint_index(index)else{returnDesktopPresentationApplyOutcome::Rejected;};self.select(self.selection.with_skin(skin),true,admit)}'
$checkedSuccessorDerivationCount = [int]((Normalize-ExecutableStructure -Text $checkedSuccessorText) -eq $expectedCheckedSuccessor)
$selectDensityStructureCount = [int]((Normalize-ExecutableStructure -Text $selectDensityText) -eq $expectedSelectDensity)
$selectDensityIfAdmittedStructureCount = [int]((Normalize-ExecutableStructure -Text $selectDensityIfAdmittedText) -eq $expectedSelectDensityIfAdmitted)
$selectSkinStructureCount = [int]((Normalize-ExecutableStructure -Text $selectSkinText) -eq $expectedSelectSkin)
$selectSkinIfAdmittedStructureCount = [int]((Normalize-ExecutableStructure -Text $selectSkinIfAdmittedText) -eq $expectedSelectSkinIfAdmitted)
$checkedSuccessorCallCount = $selectDensityStructureCount
$densityWriteCount = $selectDensityStructureCount
$revisionWriteCount = $selectDensityStructureCount
$appliedOutcomeCount = $selectDensityStructureCount
if ($presentationRevisionTypeCount -ne 1 -or $checkedSuccessorDerivationCount -ne 1 -or $selectDensityStructureCount -ne 1 -or $selectDensityIfAdmittedStructureCount -ne 1 -or $selectSkinStructureCount -ne 1 -or $selectSkinIfAdmittedStructureCount -ne 1) {
    throw 'TM-DESKTOP-DENSITY-REVISION: density revision updates must remain checked and fail closed'
}
$densityStressText = Get-RustFunctionText -Text $presentationStyleContractText -Name 'selection_is_complete_checked_and_revisioned_across_both_axes'
$normalizedDensityStress = Normalize-ExecutableStructure -Text $densityStressText
$densityStressStructureCount = [int]($normalizedDensityStress.Contains('style.select_density_index(1)') -and $normalizedDensityStress.Contains('style.select_skin_index(1)') -and $normalizedDensityStress.Contains('DesktopPresentationSelection::new(DesktopDensity::Compact,DesktopSkin::Refined)') -and $normalizedDensityStress.Contains('assert_eq!(style,before_rejection);'))
$densitySwitchLoopCount = $densityStressStructureCount
$densityAppliedAssertionCount = $densityStressStructureCount
$densityFinalPostconditionCount = $densityStressStructureCount
if ($densityStressStructureCount -ne 1) {
    throw 'TM-DESKTOP-DENSITY-STRESS: density must retain one 10,000-switch contract'
}
$densityAuthorityText = $presentationStyleExecutableText + "`n" + $densityApplyText + "`n" + $densityWireText
$densityAllowedOwnerPattern = 'Arc\s*<\s*Mutex\s*<\s*DesktopPresentationStyle\s*>\s*>'
$densityAllowedOwnerOccurrenceCount = [regex]::Matches($uiRustProductionText, 'Arc::new\(Mutex::new\(initial_presentation_style\)\)').Count
$densityAllowedOwnerWireSignatureCount = [regex]::Matches(
    $densityWireText,
    "(?s)\bfn\s+wire_presentation_density\s*\(\s*window\s*:\s*&\s*MainWindow\s*,\s*presentation_style\s*:\s*$densityAllowedOwnerPattern\s*,\s*intent_sink\s*:\s*Rc\s*<\s*dyn\s+DesktopIntentSink\s*>\s*,?\s*\)"
).Count
if ($densityAllowedOwnerOccurrenceCount -ne 1 -or $densityAllowedOwnerWireSignatureCount -ne 1) {
    throw 'TM-DESKTOP-DENSITY-NO-AUTHORITY: presentation density must retain exactly one wiring owner signature'
}
$densityAdmissionText = Get-RustFunctionText -Text $uiRustProductionText -Name 'select_presentation_density_if_admitted'
$skinAdmissionText = Get-RustFunctionText -Text $uiRustProductionText -Name 'select_presentation_skin_if_admitted'
$admissionBeforeApply = $densityAdmissionText.IndexOf('selected.select_density_index_if_admitted', [System.StringComparison]::Ordinal)
$firstDirectApply = $densityAdmissionText.IndexOf('selected.select_density_index(', [System.StringComparison]::Ordinal)
$skinAdmissionBeforeApply = $skinAdmissionText.IndexOf('selected.select_skin_index_if_admitted', [System.StringComparison]::Ordinal)
$firstDirectSkinApply = $skinAdmissionText.IndexOf('selected.select_skin_index(', [System.StringComparison]::Ordinal)
if ($admissionBeforeApply -lt 0 -or ($firstDirectApply -ge 0 -and $firstDirectApply -lt $admissionBeforeApply) -or
    $skinAdmissionBeforeApply -lt 0 -or ($firstDirectSkinApply -ge 0 -and $firstDirectSkinApply -lt $skinAdmissionBeforeApply) -or
    $densityAdmissionText -notmatch 'intent_sink\.submit\(DesktopIntent::UpdatePresentation\(selection\)\)' -or
    $skinAdmissionText -notmatch 'intent_sink\.submit\(DesktopIntent::UpdatePresentation\(selection\)\)' -or
    $skinAdmissionText.IndexOf('selected.select_skin_index_if_admitted', [System.StringComparison]::Ordinal) -lt 0 -or
    $selectDensityIfAdmittedStructureCount -ne 1 -or $selectSkinIfAdmittedStructureCount -ne 1) {
    throw 'TM-DESKTOP-DENSITY-ADMISSION: density must be admitted before any style application'
}
$backupPolicyUpdateText = Get-RustFunctionText -Text $appStateText -Name 'update_backup_policy'
$reminderPolicyUpdateText = Get-RustFunctionText -Text $appStateText -Name 'update_reminder_policy'
if (@([regex]::Matches($backupPolicyUpdateText, '\*current\.value\(\)\.portable\(\)\.presentation\(\)')).Count -ne 1 -or
    @([regex]::Matches($reminderPolicyUpdateText, '\*current\.value\(\)\.portable\(\)\.presentation\(\)')).Count -ne 1) {
    throw 'TM-DESKTOP-PRESENTATION-PRESERVATION: reminder and backup updates must preserve presentation exactly'
}
$densityFollowUpTestNameCount = @([regex]::Matches($appOperationTestsText, 'fn\s+presentation_follow_up_replaces_only_the_pending_complete_payload\s*\(')).Count
$densityLatestPayloadTestNameCount = @([regex]::Matches($appOperationTestsText, 'fn\s+ten_thousand_presentation_updates_keep_one_latest_payload\s*\(')).Count
if ($densityFollowUpTestNameCount -ne 1 -or $densityLatestPayloadTestNameCount -ne 1) {
    throw 'TM-DESKTOP-DENSITY-STRESS: one-active one-pending latest-payload proofs are required'
}
$densityFollowUpTestText = Get-RustFunctionText -Text $appOperationTestsText -Name 'presentation_follow_up_replaces_only_the_pending_complete_payload'
$densityLatestPayloadTestText = Get-RustFunctionText -Text $appOperationTestsText -Name 'ten_thousand_presentation_updates_keep_one_latest_payload'
$normalizedDensityFollowUpTest = Normalize-ExecutableStructure -Text $densityFollowUpTestText
$normalizedDensityLatestPayloadTest = Normalize-ExecutableStructure -Text $densityLatestPayloadTestText
$densityFollowUpProofs = @(
    'assert_eq!(snapshot.active_count(),1);',
    'assert_eq!(snapshot.pending_count(),1);',
    'assert_eq!(receive(&started_rx),(pending,DesktopPresentationSelection::new(DesktopDensity::Comfortable,DesktopSkin::Ember)));',
    'vec![DesktopPresentationSelection::new(DesktopDensity::Compact,DesktopSkin::Refined),DesktopPresentationSelection::new(DesktopDensity::Comfortable,DesktopSkin::Ember)]'
)
$densityLatestPayloadProofs = @(
    'forindexin0..10_000{',
    'ApplicationCommandAdmission::Queued{..}|ApplicationCommandAdmission::Coalesced{..}',
    'assert_eq!(snapshot.active_count(),1);',
    'assert_eq!(snapshot.pending_count(),1);',
    'assert_eq!(receive(&started_rx),final_selection);',
    'DesktopPresentationSelection::new(DesktopDensity::UltraCompact,DesktopSkin::Ember)'
)
if (@($densityFollowUpProofs | Where-Object { -not $normalizedDensityFollowUpTest.Contains($_) }).Count -ne 0 -or
    @($densityLatestPayloadProofs | Where-Object { -not $normalizedDensityLatestPayloadTest.Contains($_) }).Count -ne 0) {
    throw 'TM-DESKTOP-DENSITY-STRESS: one-active one-pending latest-payload proofs are required'
}
$densityAuthorityText = [regex]::Replace(
    $densityAuthorityText,
    $densityAllowedOwnerPattern,
    ''
)
$densityAuthorityText = [regex]::Replace(
    $densityAuthorityText,
    'Rc\s*<\s*dyn\s+DesktopIntentSink\s*>',
    ''
)
$densityAuthorityPatterns = [ordered]@{
    timer_delay_interval_sleep = '\b(?:slint\s*::\s*)?(?:Timer|Delay|Interval)\b|\b(?:sleep|delay)\s*\('
    worker_thread_spawn_task = '\bworker\b|\bthread\s*(?:::\s*(?:scope|spawn|Builder))?\b|\bscoped\b|\bspawn\s*\(|\btask\b|\btokio\b|\basync\b'
    query = '\b[A-Za-z_][A-Za-z0-9_]*Query[A-Za-z0-9_]*\b'
    window_create = '\bCreateWindow(?:ExW)?\b|\b(?:MainWindow|Window)\s*::\s*(?:new|builder)\b'
    queue_deque = '\b(?:VecDeque|Queue)\b'
    cache = '\b[A-Za-z_][A-Za-z0-9_]*Cache\b'
    channel = '\b(?:mpsc|sync_channel|channel|Sender|Receiver)\b'
    unsafe = '\bunsafe\b'
    retained = '\b(?:Vec|Box|HashMap|BTreeMap|HashSet|BTreeSet|BinaryHeap|Rc|RefCell|Cell|Once|OnceCell|OnceLock|Mutex|RwLock|Arc)(?:\s*(?:::)?\s*<|\s*::\s*(?:new|default|with_capacity))'
}
$densityAuthorityCategoryCounts = [ordered]@{}
foreach ($category in $densityAuthorityPatterns.Keys) {
    $densityAuthorityCategoryCounts[$category] = [regex]::Matches(
        $densityAuthorityText,
        $densityAuthorityPatterns[$category],
        [System.Text.RegularExpressions.RegexOptions]::IgnoreCase
    ).Count
}
$densityAuthorityCount = @($densityAuthorityCategoryCounts.Values | Measure-Object -Sum).Sum
if ($densityAuthorityCount -ne 0) {
    throw 'TM-DESKTOP-DENSITY-NO-AUTHORITY: presentation density must not create timer/worker/query/window or retained authority'
}
}
$workerConstructionCount = [regex]::Matches($controllerText, 'RefreshWorker::spawn\(').Count
if ($workerConstructionCount -ne 1) {
    throw 'TM-DESKTOP-CONTROLLER-WORKER: desktop controller must construct exactly one bounded refresh worker'
}
$snapshotSlotCount = [regex]::Matches(
    $productionText,
    'Arc<Mutex<Option<Arc<ProductSnapshot>>>>'
).Count
if ($snapshotSlotCount -ne 1) {
    throw 'TM-DESKTOP-CONTROLLER-SLOT: desktop and bridge must share exactly one latest product snapshot slot'
}
$bridgeEventScheduleCount = [regex]::Matches($bridgeText, 'slint::invoke_from_event_loop\(').Count
if ($bridgeEventScheduleCount -ne 1) {
    throw 'TM-DESKTOP-BRIDGE-EVENT: desktop bridge must contain exactly one event-loop scheduling site'
}
$reliableEventScheduleCount = [regex]::Matches($uiRustText, 'slint::invoke_from_event_loop\(').Count
if ($reliableEventScheduleCount -ne 1) {
    throw 'TM-DESKTOP-RELIABLE-EVENT: reliable-state delivery must contain exactly one event-loop scheduling site'
}
$eventScheduleCount = [regex]::Matches($productionText, 'slint::invoke_from_event_loop\(').Count
if ($eventScheduleCount -ne 2) {
    throw 'TM-DESKTOP-EVENT-SITES: desktop must contain exactly two bounded event-loop scheduling sites'
}
if ($bridgeText -notmatch 'window:\s*slint::Weak<MainWindow>') {
    throw 'TM-DESKTOP-BRIDGE-WEAK: desktop bridge must retain only a weak Slint window handle'
}
if ($bridgeText -match 'window:\s*MainWindow|\b(slint::Timer|std::thread|thread::spawn|thread::sleep)\b') {
    throw 'TM-DESKTOP-BRIDGE-POLLING: desktop bridge must not retain a strong window, timer, or polling thread'
}
if ($uiRustProductionText -notmatch 'window:\s*slint::Weak<MainWindow>' -or
    $uiRustProductionText -notmatch 'latest:\s*Mutex<Option<ReliableStateDelivery>>' -or
    $uiRustProductionText -notmatch 'scheduled:\s*AtomicBool') {
    throw 'TM-DESKTOP-RELIABLE-SLOT: reliable-state delivery must use one latest-only slot, one atomic gate, and a weak window'
}
$reliableQueueRemainder = $uiRustProductionText -replace 'sync_channel\(1\)', '' -replace '\bmpsc::\{SyncSender, sync_channel\}', ''
$reliableAckChannelCount = [regex]::Matches($uiRustProductionText, 'sync_channel\(1\)').Count
if ($reliableAckChannelCount -ne 1 -or
    $reliableQueueRemainder -match '\bVecDeque\b|\b(?:sync_)?channel\b') {
    throw 'TM-DESKTOP-RELIABLE-QUEUE: reliable-state delivery must not retain an unbounded or ordered event queue'
}
if ($reliableStateText -notmatch 'pub struct DesktopRecoveryReceipt' -or
    $reliableStateText -notmatch 'reconstructed_from_authoritative_source' -or
    $reliableStateText -notmatch 'non_reconstructible_domains_lost' -or
    $uiRustText -notmatch 'set_reliable_recovery_kind' -or
    $uiRustText -notmatch 'set_reliable_non_reconstructible_domains_lost' -or
    $uiText -notmatch 'Previous quota, reset-credit, reminder, and Git history is unavailable\.') {
    throw 'TM-DESKTOP-RECOVERY-RECEIPT: durable recovery loss must remain explicit and visible'
}
$uiAdapterText = $uiRustText + "`n" + $uiText
$uiProductionAdapterText = $uiRustProductionText + "`n" + $uiText
if ($uiAdapterText -match 'QueryService::|RefreshWorker::|DesktopController::|\.usage_analytics\(|\.usage_session_detail\(|\.quota_overview\(|\.benefit_overview\(') {
    throw 'TM-DESKTOP-UI-QUERY: Slint callbacks must not perform controller or query work'
}
if ($controllerText -match 'QuotaCurrentRequest::new\s*\(\s*Vec::new\(\)\s*\)') {
    throw 'TM-DESKTOP-EMPTY-FILTER-DISCOVERY: exact-empty quota filters must not be used for dashboard discovery'
}
if ($uiText -match '(?i)\b(?:text|label|title)\s*:\s*"[^"\r\n]*\b(?:5[ -]?(?:h|hour)|five[ -]?hour|weekly)\b') {
    throw 'TM-DESKTOP-FIXED-QUOTA-ROW: quota rows must be discovered dynamically'
}
if ($uiText -match '(?m)\bdashboard-(?:header-(?:tokens|cost|events)|code-(?:commits|added|removed|net|efficiency))\s*:\s*"(?:\$|\+|-|−)?[0-9]') {
    throw 'TM-DESKTOP-SEEDED-DASHBOARD: dashboard metrics must come from the immutable product snapshot'
}
if ($uiAdapterText -match '(?i)\b(?:account|workspace|window|lot|repo|repository|session|event|source)[_-]?id\b') {
    throw 'TM-DESKTOP-PRIVATE-IDENTITY: private opaque identities must not cross the UI boundary'
}
$modelsText = [System.IO.File]::ReadAllText((Join-Path $uiRoot 'models.slint'))
if ($modelsText -match '(?i)passphrase|password|confirmation') {
    throw 'TM-DESKTOP-SECRET-MODEL: passphrases must never enter a Slint list or global model'
}
if (
    $uiProductionAdapterText -match '(?i)\b(?:slint::Timer|std::thread|thread::spawn|thread::sleep)\b' -or
    $uiText -match '(?i)(?:\bTimer\s*\{|\banimate\s+[A-Za-z_-]+\b|\banimation-[A-Za-z_-]+\b)'
) {
    throw 'TM-DESKTOP-UI-POLLING: UI must remain timer animation and polling free'
}

$commandPalettePath = Join-Path $uiRoot 'components\command-palette.slint'
$commandPaletteText = [System.IO.File]::ReadAllText($commandPalettePath)
$mainUiTextForPalette = [System.IO.File]::ReadAllText((Join-Path $uiRoot 'main.slint'))
$commandPaletteQueryCap = [int]([regex]::Match($uiRustText, 'MAX_COMMAND_PALETTE_QUERY_SCALARS: usize = (\d+);').Groups[1].Value)
$commandPaletteModelCount = [regex]::Matches($mainUiTextForPalette, 'in property <\[RouteRow\]> command-palette-rows;').Count
$commandPaletteShortcutCount = [regex]::Matches(
    $mainUiTextForPalette,
    'KeyBinding\s*\{\s*keys:\s*@keys\(Control \+ K\);\s*activated\s*=>\s*\{\s*root\.open-command-palette\(\);\s*\}\s*\}'
).Count
$commandPaletteCaptureShortcutCount = [regex]::Matches(
    $mainUiTextForPalette,
    'if\s*\(event\.modifiers\.control\s*&&\s*event\.text\s*==\s*"k"\)\s*\{\s*root\.open-command-palette\(\);\s*return accept;\s*\}'
).Count
$commandPaletteHeaderOpenCount = [regex]::Matches(
    $mainUiTextForPalette,
    '(?s)Button\s*\{\s*text:\s*"Go to route";\s*accessible-label:\s*"Open route palette";\s*clicked\s*=>\s*\{\s*root\.open-command-palette\(\);\s*\}\s*\}'
).Count
$commandPaletteEscapeDismissCount = [regex]::Matches(
    $commandPaletteText,
    'if\s*\(event\.text\s*==\s*Key\.Escape\)\s*\{\s*root\.dismiss\(\);\s*return accept;\s*\}'
).Count
$commandPaletteUpMoveCount = [regex]::Matches(
    $commandPaletteText,
    'if\s*\(event\.text\s*==\s*Key\.UpArrow\)\s*\{\s*root\.move-selection\(-1\);\s*return accept;\s*\}'
).Count
$commandPaletteDownMoveCount = [regex]::Matches(
    $commandPaletteText,
    'if\s*\(event\.text\s*==\s*Key\.DownArrow\)\s*\{\s*root\.move-selection\(1\);\s*return accept;\s*\}'
).Count
$commandPaletteDefaultActionCount = [regex]::Matches(
    $commandPaletteText,
    'accessible-action-default\s*=>\s*\{\s*root\.activate-route\(route\.key\);\s*\}'
).Count
$commandPalettePointerRouteActionCount = [regex]::Matches(
    $commandPaletteText,
    'TouchArea\s*\{\s*clicked\s*=>\s*\{\s*root\.activate-route\(route\.key\);\s*\}\s*\}'
).Count
$commandPaletteEnterRouteActionCount = [regex]::Matches(
    $commandPaletteText,
    'if\s*\(event\.text\s*==\s*Key\.Return\)\s*\{\s*root\.activate-selection\(\);\s*return accept;\s*\}'
).Count
$commandPaletteMainSelectionBindingCount = [regex]::Matches(
    $mainUiTextForPalette,
    'activate-selection\s*=>\s*\{\s*root\.activate-command-palette-selection\(\);\s*\}'
).Count
$commandPaletteMainRouteBindingCount = [regex]::Matches(
    $mainUiTextForPalette,
    'activate-route\(key\)\s*=>\s*\{\s*root\.activate-command-palette-route\(key\);\s*\}'
).Count
$commandPaletteStableSelectionCount = [regex]::Matches(
    $uiRustText,
    'state\.select_stable_key\(key\)\.is_err\(\)'
).Count
$commandPaletteForbiddenMutationLabelCount = [regex]::Matches(
    $commandPaletteText,
    '(?i)\btext\s*:\s*"(?:backup|restore|import|export|rebuild|activation)'
).Count
$commandPaletteOwnerCount = [regex]::Matches(
    $commandPaletteText,
    '(?i)\b(?:Timer|thread|spawn|cache|history|worker)\b'
).Count + [regex]::Matches(
    $uiRustText,
    'CommandPalette(?:Worker|Owner|Cache|History)'
).Count
$commandPaletteRouteOnly = (
    $commandPaletteDefaultActionCount -eq 1 -and
    $commandPalettePointerRouteActionCount -eq 1 -and
    $commandPaletteEnterRouteActionCount -eq 1 -and
    $commandPaletteMainSelectionBindingCount -eq 1 -and
    $commandPaletteMainRouteBindingCount -eq 1 -and
    $commandPaletteStableSelectionCount -eq 1 -and
    $commandPaletteForbiddenMutationLabelCount -eq 0
)
$commandPaletteShortcutNavigation = (
    $commandPaletteShortcutCount -eq 1 -and
    $commandPaletteCaptureShortcutCount -eq 1 -and
    $commandPaletteHeaderOpenCount -eq 1 -and
    $commandPaletteEscapeDismissCount -eq 1 -and
    $commandPaletteUpMoveCount -eq 1 -and
    $commandPaletteDownMoveCount -eq 1 -and
    $commandPaletteEnterRouteActionCount -eq 1
)
if ($commandPaletteQueryCap -ne 64 -or
    $uiRustText -notmatch 'value\s*\.chars\(\)\s*\.take\(MAX_COMMAND_PALETTE_QUERY_SCALARS\)' -or
    $uiRustText -notmatch 'window\.set_command_palette_rows\(model\(rows\)\)' -or
    $commandPaletteModelCount -ne 1 -or
    [regex]::Matches($mainUiTextForPalette, 'command-palette-rows').Count -ne 2) {
    throw 'TM-DESKTOP-COMMAND-PALETTE-BOUND: command palette must retain one replace-only filtered route model and a 64-scalar query'
}
if (-not $commandPaletteShortcutNavigation) {
    throw 'TM-DESKTOP-COMMAND-PALETTE-SHORTCUT: command palette must expose the exact Ctrl+K shortcut and bounded keyboard routing'
}
if (-not $commandPaletteRouteOnly -or
    $commandPaletteText -notmatch 'No matching routes' -or
    $commandPaletteText -notmatch 'accessible-label:\s*route\.label \+ ", " \+ route\.state') {
    throw 'TM-DESKTOP-COMMAND-PALETTE-ROUTE-ONLY: command palette must remain accessible, explicit on no match, and route-only'
}
if ($mainUiTextForPalette -notmatch '(?s)shell-focus := FocusScope \{.*?RoutePalette \{' -or
    $mainUiTextForPalette -notmatch '(?s)if root\.in-app-notification-visible: InAppNotificationPanel \{.*?\}\s*\}\s*RoutePalette \{' -or
    $commandPaletteText -notmatch '(?s)palette-focus := FocusScope \{.*?search-focus := FocusScope \{.*?search := LineEdit') {
    throw 'TM-DESKTOP-COMMAND-PALETTE-OVERLAY: palette overlay and focus scopes must be top-level and ancestral'
}
if ($commandPaletteOwnerCount -ne 0) {
    throw 'TM-DESKTOP-COMMAND-PALETTE-NO-OWNER: command palette must not add a timer, worker, query, cache, or owner'
}

$inAppNotificationPath = Join-Path $sourceRoot 'in_app_notification.rs'
$inAppNotificationText = [System.IO.File]::ReadAllText($inAppNotificationPath)
$inAppPanelText = [System.IO.File]::ReadAllText(
    (Join-Path $uiRoot 'components\in-app-notification-panel.slint')
)
$mainUiTextForInApp = [System.IO.File]::ReadAllText((Join-Path $uiRoot 'main.slint'))
if ($inAppNotificationText -notmatch 'pub const MAX_DESKTOP_IN_APP_NOTIFICATIONS: usize = 256;' -or
    $inAppNotificationText -notmatch 'rows\.len\(\) > MAX_DESKTOP_IN_APP_NOTIFICATIONS' -or
    $inAppNotificationText -notmatch 'if rows\.is_empty\(\)') {
    throw 'TM-DESKTOP-IN-APP-BOUND: presentation must retain exactly one to 256 rows'
}
$inAppModelCount = [regex]::Matches(
    $mainUiTextForInApp,
    'property\s*<\[InAppNotificationRow\]>\s+in-app-notification-[A-Za-z0-9_-]+'
).Count
if ($inAppModelCount -ne 1 -or
    [regex]::Matches($uiRustText, 'set_in_app_notification_rows\(model\(rows\)\)').Count -ne 1) {
    throw 'TM-DESKTOP-IN-APP-MODEL: presentation must own one transient notification model'
}
$applyFunction = [regex]::Match(
    $uiRustText,
    '(?s)pub\(crate\) fn apply_in_app_notification_batch\(.*?\r?\n\}\r?\n\r?\nfn notification_coverage_label'
).Value
$rowsIndex = $applyFunction.IndexOf(
    'window.set_in_app_notification_rows(model(rows));',
    [System.StringComparison]::Ordinal
)
$countIndex = $applyFunction.IndexOf(
    'window.set_in_app_notification_count_label(count_label.into());',
    [System.StringComparison]::Ordinal
)
$visibleIndex = $applyFunction.IndexOf(
    'window.set_in_app_notification_visible(true);',
    [System.StringComparison]::Ordinal
)
$verifiedIndex = $applyFunction.IndexOf(
    'window.get_in_app_notification_visible()',
    [System.StringComparison]::Ordinal
)
if ([string]::IsNullOrWhiteSpace($applyFunction) -or $rowsIndex -lt 0 -or
    $countIndex -le $rowsIndex -or $visibleIndex -le $countIndex -or
    $verifiedIndex -le $visibleIndex) {
    throw 'TM-DESKTOP-IN-APP-APPLY: model count and visibility must be applied and verified in order'
}
$successfulApplyCount = [regex]::Matches(
    $inAppNotificationText,
    '(?s)if apply_in_app_notification_batch\(&window, batch\) \{\s*NotificationDeliveryOutcome::Presented'
).Count
$runNotificationFunction = [regex]::Match(
    $inAppNotificationText,
    '(?s)fn run\(.*?\r?\n    \}\r?\n\r?\n    fn record_schedule_error'
).Value
$readyBeforeReceiptCount = [regex]::Matches(
    $runNotificationFunction,
    '(?s)let presented = match self\.delivery\.deliver\(&batch\).*?self\.scheduled\.store\(false, Ordering::Release\);\s*if presented \{\s*receipt\.presented\(\);\s*\} else \{\s*receipt\.failed\(\);'
).Count
$failedDeliveryCount = [regex]::Matches(
    $runNotificationFunction,
    '(?s)NotificationDeliveryOutcome::(?:Stale|WindowClosed|StateUnavailable) => \{.*?false\s*\}'
).Count
if ($successfulApplyCount -ne 1 -or $readyBeforeReceiptCount -ne 1 -or
    $failedDeliveryCount -ne 3 -or
    [regex]::Matches($inAppNotificationText, 'receipt\.presented\(\);').Count -ne 1 -or
    [regex]::Matches($inAppNotificationText, 'receipt\.failed\(\);').Count -ne 2) {
    throw 'TM-DESKTOP-IN-APP-RECEIPT: visible apply and bridge readiness must precede Presented while every callback failure fails'
}
if ($applyFunction -notmatch '\{benefit_label\}\. \{kind_label\}, quantity \{quantity_label\}') {
    throw 'TM-DESKTOP-IN-APP-ACCESSIBILITY: accessible rows must include the visible benefit and kind labels'
}
$inAppEpochGuardCount = 0
if ($inAppNotificationText -match 'self\.epochs\.active\.load\(Ordering::Acquire\) != self\.epoch' -and
    $inAppNotificationText -match 'let epoch = epochs\.activate\(\)\?;' -and
    $inAppNotificationText -match 'self\.epochs\.deactivate\(self\.epoch\);') {
    $inAppEpochGuardCount = 1
}
if ($inAppEpochGuardCount -ne 1) {
    throw 'TM-DESKTOP-IN-APP-EPOCH: presentation must use one checked independently invalidated epoch'
}
$inAppPublicValue = [regex]::Match(
    $inAppNotificationText,
    '(?s)pub struct DesktopInAppNotification\s*\{.*?\r?\n\}'
).Value
if ($inAppPublicValue -match '(?i)\b(?:delivery|provider|account|workspace|scope|lot|window|target|receipt|activation)[_-]?id\b|\b(?:absolute_)?path\b') {
    throw 'TM-DESKTOP-IN-APP-IDENTITY: presentation value must not expose private identity or paths'
}
if ($inAppNotificationText -match '\b(?:VecDeque|sync_channel|std::thread|thread::spawn|thread::sleep|slint::Timer)\b' -or
    $inAppPanelText -match '(?i)(?:\bTimer\s*\{|\banimate\s+[A-Za-z_-]+\b|animation-[A-Za-z_-]+|auto[-_]?hide)') {
    throw 'TM-DESKTOP-IN-APP-OWNER: presentation must not add a queue timer worker polling or auto-hide owner'
}
$fixedUpstreamAttribution = 'WhereMyTokens and ccusage are pinned external MIT references, not runtime dependencies.'
$legacyProductBoundary = $uiAdapterText.Replace($fixedUpstreamAttribution, '')
if ($legacyProductBoundary -match '(?i)\b(?:WhereMyTokens|WhereMyToken|WhereMyTokens)\b') {
    throw 'TM-DESKTOP-LEGACY-PRODUCT: production UI must contain only TokenMaster product identity'
}

$dashboardPath = Join-Path $sourceRoot 'dashboard.rs'
$dashboardText = [System.IO.File]::ReadAllText($dashboardPath)
$dashboardBounds = [ordered]@{
    DESKTOP_DASHBOARD_SECTION_COUNT = 6
    MAX_DASHBOARD_QUOTA_ROWS = 32
    MAX_DASHBOARD_BENEFIT_SCOPES = 32
    MAX_DASHBOARD_TREND_POINTS = 240
    MAX_DASHBOARD_SESSIONS = 12
    DASHBOARD_ACTIVITY_ROWS = 8
    MAX_DASHBOARD_MODELS = 12
    MAX_DASHBOARD_REPOSITORIES = 32
}
foreach ($bound in $dashboardBounds.GetEnumerator()) {
    $pattern = "pub const $([regex]::Escape($bound.Key)): usize = $($bound.Value);"
    if ($dashboardText -notmatch $pattern) {
        throw "TM-DESKTOP-DASHBOARD-BOUND: $($bound.Key) drifted"
    }
}
foreach ($requiredBoundUse in @(
    '\.take\(MAX_DASHBOARD_QUOTA_ROWS\)',
    '\.take\(MAX_DASHBOARD_BENEFIT_SCOPES\)',
    '\.take\(MAX_DASHBOARD_TREND_POINTS\)',
    '\.take\(MAX_DASHBOARD_SESSIONS\)',
    '\.take\(MAX_DASHBOARD_MODELS\)',
    '\.take\(MAX_DASHBOARD_REPOSITORIES\)'
)) {
    if ($dashboardText -notmatch $requiredBoundUse) {
        throw "TM-DESKTOP-DASHBOARD-BOUND: missing bounded projection $requiredBoundUse"
    }
}
$dashboardProjectionCallCount = [regex]::Matches($uiRustText, 'apply_dashboard_projection\(').Count
if ($dashboardProjectionCallCount -ne 2) {
    throw 'TM-DESKTOP-DASHBOARD-REBUILD: dashboard models must not rebuild during route-only selection'
}
$compactWidgetPath = Join-Path $uiRoot 'views\compact-widget-view.slint'
$compactWidgetText = [System.IO.File]::ReadAllText($compactWidgetPath)
$compactWidgetQuotaPropertyCount = [regex]::Matches(
    $mainUiTextForPalette,
    'in property <\[DashboardQuotaRow\]> [a-z][a-z0-9-]*;'
).Count
$compactWidgetQuotaBindingCount = [regex]::Matches(
    $mainUiTextForPalette,
    'quotas:\s*root\.dashboard-quota-rows;'
).Count
$compactWidgetGeometrySlotCount = [regex]::Matches(
    $uiRustText,
    'normal_size:\s*Option<slint::PhysicalSize>'
).Count
$compactWidgetOwnerCount = [regex]::Matches(
    $compactWidgetText,
    '(?i)\b(?:Timer|thread|spawn|cache|history|worker|query|snapshot|controller)\b'
).Count + [regex]::Matches(
    $uiRustText,
    'CompactWidget(?:Worker|Query|Cache|Snapshot|Controller)'
).Count
if ($compactWidgetQuotaPropertyCount -ne 1 -or
    $compactWidgetQuotaBindingCount -ne 2 -or
    $compactWidgetText -notmatch 'in property <\[DashboardQuotaRow\]> quotas;' -or
    $compactWidgetText -notmatch 'for quota in root\.quotas:\s*CompactQuotaRow' -or
    $compactWidgetText -notmatch 'if !root\.quota\.ratio-known:\s*Text' -or
    $compactWidgetText -notmatch 'Usage ratio unavailable' -or
    $compactWidgetText -match '(?i)\b(?:5\s*-?\s*hour|five\s*-?\s*hour|weekly)\b') {
    throw 'TM-DESKTOP-COMPACT-QUOTA: compact mode must reuse all bounded dynamic quota rows and keep unknown ratio explicit'
}
if ($mainUiTextForPalette -notmatch 'compact-view := CompactWidgetView' -or
    $mainUiTextForPalette -match '(?s)if\s+[^:]+:\s*CompactWidgetView' -or
    $mainUiTextForPalette -notmatch 'visible:\s*!root\.compact-widget-visible;' -or
    $mainUiTextForPalette -notmatch 'return-dashboard\s*=>\s*\{\s*root\.select-route\("dashboard"\);\s*\}' -or
    $compactWidgetText -notmatch 'accessible-label:\s*"Return to Dashboard";' -or
    $compactWidgetText -notmatch 'forward-focus:\s*return-button;') {
    throw 'TM-DESKTOP-COMPACT-ROUTE: compact mode must remain one always-mounted same-window route with an accessible Dashboard return'
}
if ($compactWidgetGeometrySlotCount -ne 1 -or
    $uiRustText -notmatch 'const COMPACT_WINDOW_WIDTH: f32 = 420\.0;' -or
    $uiRustText -notmatch 'const COMPACT_WINDOW_HEIGHT: f32 = 560\.0;' -or
    $uiRustText -notmatch 'slint::LogicalSize::new\(' -or
    $uiRustText -notmatch 'mode\.normal_size = Some' -or
    $uiRustText -notmatch 'mode\.normal_size\s*\.take\(\)') {
    throw 'TM-DESKTOP-COMPACT-GEOMETRY: compact mode must use one logical-size transition and one bounded restore slot'
}
if ($compactWidgetOwnerCount -ne 0) {
    throw 'TM-DESKTOP-COMPACT-NO-OWNER: compact mode must add no query snapshot worker timer cache or controller owner'
}
$trayAssetPath = Join-Path $uiRoot 'assets\tokenmaster-tray-color-32.svg'
$shellPath = Join-Path $sourceRoot 'shell.rs'
$nativeTrayPath = Join-Path $sourceRoot 'native_tray.rs'
foreach ($requiredTrayFile in @($trayAssetPath, $shellPath, $nativeTrayPath)) {
    if (-not (Test-Path -LiteralPath $requiredTrayFile)) {
        throw 'TM-DESKTOP-TRAY-BOUNDARY: production tray files are incomplete'
    }
}
$legacyTrayPath = Join-Path $uiRoot 'tray.slint'
if (Test-Path -LiteralPath $legacyTrayPath) {
    throw 'TM-DESKTOP-TRAY-SURFACE: Slint SystemTrayIcon must not own the production tray'
}
$nativeTrayText = [System.IO.File]::ReadAllText($nativeTrayPath)
$shellText = [System.IO.File]::ReadAllText($shellPath)
$trayComponentCount = [regex]::Matches(
    $nativeTrayText,
    'CreateWindowExW\('
).Count
$trayIntentCount = [regex]::Matches(
    $shellText,
    'Self::(?:Show|Hide|OpenCompact|OpenDashboard|Quit),'
).Count
$trayRouterSlotCount = [regex]::Matches(
    $shellText,
    'sink:\s*RefCell<Option<Rc<dyn DesktopLifecycleIntentSink>>>'
).Count
$trayCloseHandlerCount = [regex]::Matches(
    $uiRustProductionText,
    'on_close_requested\(move \|\|'
).Count
$trayOwnerCount = [regex]::Matches(
    $nativeTrayText,
    'static OWNER_ACTIVE: AtomicBool'
).Count
$trayPollingSurfaceCount = [regex]::Matches(
    $nativeTrayText,
    '(?i)\b(?:Timer|sleep|interval|poll|thread::spawn)\b'
).Count
$trayExplorerRecoveryCount = [regex]::Matches(
    $nativeTrayText,
    'RegisterWindowMessageW\(w!\("TaskbarCreated"\)\)'
).Count
$trayReAddCheckCount = [regex]::Matches(
    $nativeTrayText,
    'let restored = unsafe \{ Shell_NotifyIconW\(NIM_ADD, &data\) \}\.as_bool\(\);'
).Count
$trayCallbackBindingCount = [regex]::Matches(
    $nativeTrayText,
    'let installed = unsafe \{ GetWindowLongPtrW\(inner\.hwnd, GWLP_USERDATA\) \};[\s\S]{0,256}?if installed != callback_state as isize'
).Count
$trayCallbackBindingOrderCount = [regex]::Matches(
    $nativeTrayText,
    'SetWindowLongPtrW\([\s\S]{0,384}?let installed = unsafe \{ GetWindowLongPtrW\(inner\.hwnd, GWLP_USERDATA\) \};[\s\S]{0,768}?Shell_NotifyIconW\(NIM_ADD, &data\)'
).Count
$trayFocusCount = [regex]::Matches(
    $nativeTrayText,
    'SetForegroundWindow\(hwnd\)\.as_bool\(\)'
).Count
$trayIconHash = (Get-FileHash -LiteralPath $trayAssetPath -Algorithm SHA256).Hash
if ($trayComponentCount -ne 1 -or $nativeTrayText -match '\bHWND_MESSAGE\b' -or
    $nativeTrayText -notmatch 'WS_EX_TOOLWINDOW' -or $nativeTrayText -notmatch 'WS_POPUP' -or
    [regex]::Matches($nativeTrayText, 'event == WM_LBUTTONUP[\s\S]{0,128}?DesktopLifecycleIntent::Show').Count -ne 1) {
    throw 'TM-DESKTOP-TRAY-SURFACE: one hidden top-level native tray owner with click-to-show is required'
}
if ($trayIntentCount -ne 5 -or $trayRouterSlotCount -ne 1 -or
    [regex]::Matches($nativeTrayText, 'COMMAND_(?:SHOW|DASHBOARD|COMPACT|HIDE|QUIT) => Some\(DesktopLifecycleIntent::').Count -ne 5) {
    throw 'TM-DESKTOP-TRAY-INTENT: tray must use one queue-free router slot and exactly five typed lifecycle intents'
}
if ([regex]::Matches($uiRustProductionText, 'tray:\s*RefCell<Option<DesktopNativeTrayOwner>>').Count -ne 1 -or
    [regex]::Matches($uiRustProductionText, 'DesktopNativeTrayOwner::new\(').Count -ne 1 -or
    $trayCloseHandlerCount -ne 1 -or $trayOwnerCount -ne 1 -or $trayPollingSurfaceCount -ne 0 -or
    $uiRustProductionText -notmatch 'DesktopTrayAvailability::Unavailable' -or
    $uiRustProductionText -notmatch 'DesktopCloseEffect::Quit[\s\S]{0,128}?slint::quit_event_loop\(\)') {
    throw 'TM-DESKTOP-TRAY-LIFECYCLE: tray must remain one deferred owner with fail-safe close and no polling'
}
if ($trayExplorerRecoveryCount -ne 1 -or $trayReAddCheckCount -ne 1 -or
    $trayCallbackBindingCount -ne 1 -or $trayCallbackBindingOrderCount -ne 1 -or
    $nativeTrayText -notmatch 'inner\.set_available\(restored\);' -or
    $nativeTrayText -notmatch 'if !available \{[\s\S]{0,128}?DesktopLifecycleIntent::Show' -or
    $trayFocusCount -ne 1) {
    throw 'TM-DESKTOP-TRAY-RECOVERY: Explorer recreation failure must be checked and surface the focused main window'
}
if ($trayIconHash -ne '1782E746EFBB423DF3252FD76B9E9E7135416DA966DF0C5652588AC29C0A6246') {
    throw 'TM-DESKTOP-TRAY-ASSET: production tray icon hash drifted'
}
$historyPath = Join-Path $sourceRoot 'history.rs'
$historyText = [System.IO.File]::ReadAllText($historyPath)
if ($historyText -notmatch 'pub const MAX_HISTORY_DAYS: usize = 30;' -or
    $historyText -notmatch '\.take\(MAX_HISTORY_DAYS\)') {
    throw 'TM-DESKTOP-HISTORY-BOUND: history projection must retain at most thirty daily rows'
}
if ($controllerText -notmatch 'pub const HISTORY_DAYS: u16 = 30;' -or
    $controllerText -notmatch 'UsageRange::recent_days\(Self::HISTORY_DAYS\)') {
    throw 'TM-DESKTOP-HISTORY-REQUEST: history query must remain one fixed bounded recent-days request'
}
$historyProjectionCallCount = [regex]::Matches($uiRustText, 'apply_history_projection\(').Count
if ($historyProjectionCallCount -ne 2) {
    throw 'TM-DESKTOP-HISTORY-REBUILD: history models must not rebuild during route-only selection'
}
$historyModelReplacementCount = [regex]::Matches(
    $uiRustText,
    'set_history_day_rows\(model\(rows\)\)'
).Count
if ($historyModelReplacementCount -ne 1) {
    throw 'TM-DESKTOP-HISTORY-MODEL: history must have one bounded model replacement site'
}
$modelsProjectionPath = Join-Path $sourceRoot 'models.rs'
$modelsProjectionText = [System.IO.File]::ReadAllText($modelsProjectionPath)
if ($modelsProjectionText -notmatch 'pub const MAX_MODEL_ROWS: usize = 64;' -or
    $modelsProjectionText -notmatch '\.take\(MAX_MODEL_ROWS\)' -or
    $modelsProjectionText -notmatch 'breakdown\.truncated\(\) \|\| breakdown\.items\(\)\.len\(\) > MAX_MODEL_ROWS') {
    throw 'TM-DESKTOP-MODELS-BOUND: Models projection must preserve backend truncation and retain at most sixty-four rows'
}
$analyticsQueryCallCount = [regex]::Matches($controllerText, 'source\.usage_analytics\(').Count
$recentModelsRequestPattern = '(?s)let history = UsageAnalyticsRequest::new\(\s*UsageRange::recent_days\(Self::HISTORY_DAYS\).*?UsageSeriesSelection::Daily,\s*Vec::new\(\),\s*vec!\[\s*UsageBreakdownKind::Model,\s*UsageBreakdownKind::Project,?\s*\],\s*\)'
if ($analyticsQueryCallCount -ne 2 -or $controllerText -notmatch $recentModelsRequestPattern) {
    throw 'TM-DESKTOP-MODELS-REQUEST: Models and Projects must share the one fixed recent analytics request without a third query'
}
$modelsProjectionCallCount = [regex]::Matches($uiRustText, 'apply_models_projection\(').Count
if ($modelsProjectionCallCount -ne 2) {
    throw 'TM-DESKTOP-MODELS-REBUILD: Models rows must not rebuild during route-only selection'
}
$modelsModelReplacementCount = [regex]::Matches(
    $uiRustText,
    'set_model_usage_rows\(model\(rows\)\)'
).Count
if ($modelsModelReplacementCount -ne 1) {
    throw 'TM-DESKTOP-MODELS-MODEL: Models must have one bounded model replacement site'
}
$mainUiText = [System.IO.File]::ReadAllText((Join-Path $uiRoot 'main.slint'))
$modelsViewText = [System.IO.File]::ReadAllText((Join-Path $uiRoot 'views\models-view.slint'))
$requiredModelsViewPatterns = @(
    'if root\.models-visible: ModelsView',
    '!root\.models-visible',
    'out property <bool> narrow:',
    'if root\.narrow:',
    'if !root\.narrow:',
    'model\.input-label',
    'model\.cached-label',
    'model\.output-label',
    'model\.reasoning-label',
    'model\.total-label',
    'model\.cost-label',
    'model\.cost-evidence-label',
    'model\.event-label',
    'root\.total-availability',
    'root\.cost-availability',
    'Text \{ text: "Relative";',
    'accessible-label:'
)
foreach ($requiredPattern in $requiredModelsViewPatterns) {
    $viewBoundary = $mainUiText + "`n" + $modelsViewText
    if ($viewBoundary -notmatch $requiredPattern) {
        throw "TM-DESKTOP-MODELS-VIEW: missing responsive Models contract $requiredPattern"
    }
}
$projectsProjectionPath = Join-Path $sourceRoot 'projects.rs'
$projectsProjectionText = [System.IO.File]::ReadAllText($projectsProjectionPath)
if ($projectsProjectionText -notmatch 'pub const MAX_PROJECT_ROWS: usize = 32;' -or
    $projectsProjectionText -notmatch '\.take\(MAX_PROJECT_ROWS\)' -or
    $projectsProjectionText -notmatch 'breakdown\.truncated\(\) \|\| breakdown\.items\(\)\.len\(\) > MAX_PROJECT_ROWS') {
    throw 'TM-DESKTOP-PROJECTS-BOUND: Projects projection must preserve backend truncation and retain at most thirty-two rows'
}
$gitQueryCallCount = [regex]::Matches($controllerText, 'source\.git_output\(').Count
$todayGitRequestPattern = '(?s)let git = GitOutputRequest::new\(\s*UsageRange::today\(\),\s*WeekStart::Monday,\s*Vec::new\(\),\s*Self::MAX_REPOSITORIES,\s*\)'
if ($gitQueryCallCount -ne 1 -or $controllerText -notmatch $todayGitRequestPattern) {
    throw 'TM-DESKTOP-PROJECTS-REQUEST: Projects must reuse one bounded UTC-today Git request'
}
if ($projectsProjectionText -notmatch 'alias\.as_str\(\) == project' -or
    $projectsProjectionText -match '(?i)contains\(project\)|starts_with\(project\)|ends_with\(project\)|to_lowercase\(\)') {
    throw 'TM-DESKTOP-PROJECTS-JOIN: Projects must join Git only by an exact safe alias'
}
if ($projectsProjectionText -notmatch 'self\.cost = Some\(cost\)' -or
    $projectsProjectionText -match 'self\.cost\s*=\s*self\.cost.*checked_add|efficiency_cost\.checked_add|efficiency_usage\.checked_add') {
    throw 'TM-DESKTOP-PROJECTS-EFFICIENCY: same-alias repositories must count one project cost exactly once'
}
$projectsProjectionCallCount = [regex]::Matches($uiRustText, 'apply_projects_projection\(').Count
if ($projectsProjectionCallCount -ne 2) {
    throw 'TM-DESKTOP-PROJECTS-REBUILD: Projects rows must not rebuild during route-only selection'
}
$projectsModelReplacementCount = [regex]::Matches(
    $uiRustText,
    'set_project_usage_rows\(model\(rows\)\)'
).Count
if ($projectsModelReplacementCount -ne 1) {
    throw 'TM-DESKTOP-PROJECTS-MODEL: Projects must have one bounded model replacement site'
}
$projectsViewText = [System.IO.File]::ReadAllText((Join-Path $uiRoot 'views\projects-view.slint'))
$projectsViewBoundary = $mainUiText + "`n" + $projectsViewText + "`n" + $uiRustText
foreach ($requiredPattern in @(
    'if root\.projects-visible: ProjectsView',
    '!root\.projects-visible',
    'out property <bool> narrow:',
    'if root\.narrow:',
    'if !root\.narrow:',
    'Recent usage',
    'Today code',
    'usage-range-label',
    'code-range-label',
    'project\.input-label',
    'project\.cached-label',
    'project\.output-label',
    'project\.reasoning-label',
    'project\.total-label',
    'project\.cost-label',
    'project\.cost-evidence-label',
    'project\.commits-label',
    'project\.added-label',
    'project\.removed-label',
    'project\.net-label',
    'project\.efficiency-label',
    'project\.code-status-label',
    'project\.efficiency-reason-label',
    'project\.code-evidence-label',
    '"repository_not_linked" => "Not linked"',
    'Cost / 100 added product-code lines',
    'added product-code lines',
    'Text \{ text: "Relative";',
    'accessible-label:.*project\.code-status-label.*project\.efficiency-reason-label'
)) {
    if ($projectsViewBoundary -cnotmatch $requiredPattern) {
        throw "TM-DESKTOP-PROJECTS-VIEW: missing responsive Projects contract $requiredPattern"
    }
}
$projectPublicText = @(
    [regex]::Match($projectsProjectionText, '(?s)pub struct DesktopProjectUsageRow\s*\{.*?\r?\n\}').Value
    [regex]::Match($projectsProjectionText, '(?s)pub struct DesktopProjectsProjection\s*\{.*?\r?\n\}').Value
) -join "`n"
if ($projectPublicText -match '(?i)\b(?:repository|association|dataset|provider|profile|account|session|source|event)[_-]?id\b|\b(?:absolute_)?path\b|\bkey\b|\bcursor\b') {
    throw 'TM-DESKTOP-PROJECTS-IDENTITY: private identity or path crossed the Projects projection boundary'
}
$activityProjectionPath = Join-Path $sourceRoot 'activity.rs'
$activityProjectionText = [System.IO.File]::ReadAllText($activityProjectionPath)
if ($activityProjectionText -notmatch 'pub const MAX_ACTIVITY_ROWS: usize = 12;' -or
    $activityProjectionText -notmatch '\.take\(MAX_ACTIVITY_ROWS\)' -or
    $activityProjectionText -notmatch 'page\.has_more\(\) \|\| truncated') {
    throw 'TM-DESKTOP-ACTIVITY-BOUND: Activity projection must preserve page incompleteness and retain at most twelve rows'
}
$activityQueryCallCount = [regex]::Matches($controllerText, 'source\.latest_activity\(').Count
if ($activityQueryCallCount -ne 1 -or
    $controllerText -notmatch 'pub const MAX_DASHBOARD_ROWS: usize = 12;' -or
    $controllerText -notmatch 'activity: LatestActivityRequest::first\(overview_page_size\)') {
    throw 'TM-DESKTOP-ACTIVITY-REQUEST: Activity must reuse one bounded first-page request on the existing worker'
}
$activityProjectionCallCount = [regex]::Matches($uiRustText, 'apply_activity_route_projection\(').Count
if ($activityProjectionCallCount -ne 2) {
    throw 'TM-DESKTOP-ACTIVITY-REBUILD: Activity rows must not rebuild during route-only selection'
}
$activityModelReplacementCount = [regex]::Matches(
    $uiRustText,
    'set_recent_activity_rows\(model\(rows\)\)'
).Count
if ($activityModelReplacementCount -ne 1) {
    throw 'TM-DESKTOP-ACTIVITY-MODEL: Activity must have one bounded model replacement site'
}
$activityViewText = [System.IO.File]::ReadAllText((Join-Path $uiRoot 'views\activity-view.slint'))
$activityViewBoundary = $mainUiText + "`n" + $activityViewText + "`n" + $uiRustText
foreach ($requiredPattern in @(
    'if root\.activity-visible: ActivityView',
    '!root\.activity-visible',
    'out property <bool> narrow:',
    'if root\.narrow:',
    'if !root\.narrow:',
    'Recent activity',
    'UTC timestamps',
    'More activity available',
    'set_activity_page_available\(activity\.has_more\(\)\.is_some\(\)\)',
    'No activity events in the available page',
    'format_timestamp_utc\(row\.timestamp_seconds\(\), row\.timestamp_nanos\(\)\)',
    'item\.input-label',
    'item\.cached-label',
    'item\.output-label',
    'item\.reasoning-label',
    'item\.total-label',
    'accessible-label:.*item\.input-label.*item\.cached-label.*item\.output-label.*item\.reasoning-label.*item\.total-label'
)) {
    if ($activityViewBoundary -cnotmatch $requiredPattern) {
        throw "TM-DESKTOP-ACTIVITY-VIEW: missing responsive Activity contract $requiredPattern"
    }
}
$activityPublicText = @(
    [regex]::Match($activityProjectionText, '(?s)pub struct DesktopRecentActivityRow\s*\{.*?\r?\n\}').Value
    [regex]::Match($activityProjectionText, '(?s)pub struct DesktopActivityProjection\s*\{.*?\r?\n\}').Value
) -join "`n"
if ($activityPublicText -match '(?i)\b(?:scope|provider|profile|account|session|source|event|cursor|fingerprint|dataset|project|path|key|id)(?:[_-]?id)?\b\s*:') {
    throw 'TM-DESKTOP-ACTIVITY-IDENTITY: private identity or provenance crossed the Activity projection boundary'
}
if ($activityProjectionText -match '\.(?:scope|provider|profile|account|session|source|event_id|cursor|fingerprint|dataset|project|path|key|id)\(\)') {
    throw 'TM-DESKTOP-ACTIVITY-IDENTITY: Activity projection must not read private identity or provenance fields'
}
if ($activityViewBoundary -match '(?i)\b(?:rhythm|heatmap|day-of-week|hourly)\b') {
    throw 'TM-DESKTOP-ACTIVITY-RHYTHM: Recent activity must not claim an unimplemented rhythm or heatmap aggregate'
}
$notificationsProjectionPath = Join-Path $sourceRoot 'notifications.rs'
$notificationsProjectionText = [System.IO.File]::ReadAllText($notificationsProjectionPath)
$notificationBounds = [ordered]@{
    MAX_NOTIFICATION_SCOPES = 32
    MAX_NOTIFICATION_LOTS = 256
    MAX_NOTIFICATION_LEADS = 8
}
foreach ($bound in $notificationBounds.GetEnumerator()) {
    $pattern = "pub const $([regex]::Escape($bound.Key)): usize = $($bound.Value);"
    if ($notificationsProjectionText -notmatch $pattern) {
        throw "TM-DESKTOP-NOTIFICATIONS-BOUND: $($bound.Key) drifted"
    }
}
foreach ($requiredBoundUse in @(
    '\.take\(MAX_NOTIFICATION_SCOPES\)',
    '\.take\(MAX_NOTIFICATION_LEADS\)',
    'lots\.len\(\) == MAX_NOTIFICATION_LOTS'
)) {
    if ($notificationsProjectionText -notmatch $requiredBoundUse) {
        throw "TM-DESKTOP-NOTIFICATIONS-BOUND: missing bounded projection $requiredBoundUse"
    }
}
$benefitQueryCallCount = [regex]::Matches($controllerText, 'source\.benefit_overview\(').Count
if ($benefitQueryCallCount -ne 1 -or
    $controllerText -notmatch 'source\.benefit_overview\(BenefitOverviewRequest::new\(\)\)') {
    throw 'TM-DESKTOP-NOTIFICATIONS-REQUEST: Notifications must reuse one bounded all-current benefit overview'
}
$notificationsProjectionCallCount = [regex]::Matches(
    $uiRustText,
    'apply_notifications_projection\('
).Count
if ($notificationsProjectionCallCount -ne 2) {
    throw 'TM-DESKTOP-NOTIFICATIONS-REBUILD: Notifications models must not rebuild during route-only selection'
}
$notificationScopeModelReplacementCount = [regex]::Matches(
    $uiRustText,
    'set_reminder_scope_rows\(model\(scope_rows\)\)'
).Count
$notificationLotModelReplacementCount = [regex]::Matches(
    $uiRustText,
    'set_benefit_lot_rows\(model\(lot_rows\)\)'
).Count
if ($notificationScopeModelReplacementCount -ne 1 -or
    $notificationLotModelReplacementCount -ne 1) {
    throw 'TM-DESKTOP-NOTIFICATIONS-MODEL: Notifications must have one replacement site for each bounded model'
}
$notificationsViewText = [System.IO.File]::ReadAllText((Join-Path $uiRoot 'views\notifications-view.slint'))
$notificationsViewBoundary = $mainUiText + "`n" + $notificationsViewText + "`n" +
    $uiRustText + "`n" + $notificationsProjectionText
foreach ($requiredPattern in @(
    'if root\.notifications-visible: NotificationsView',
    '!root\.notifications-visible',
    'out property <bool> narrow:',
    'if root\.narrow:',
    'if !root\.narrow:',
    'Expiry reminders',
    'effective in-app coverage',
    'scope\.coverage-label',
    'scope\.source-label',
    'scope\.leads-label',
    'scope\.next-due-label',
    'scope\.nearest-expiry-label',
    'scope\.evidence-label',
    'scope\.warning-label',
    'lot\.kind-label',
    'lot\.quantity-label',
    'lot\.state-label',
    'lot\.expiry-label',
    'lot\.evidence-label',
    'accessible-label:.*lot\.kind-label.*lot\.quantity-label.*lot\.state-label.*lot\.expiry-label.*lot\.evidence-label'
)) {
    if ($notificationsViewBoundary -cnotmatch $requiredPattern) {
        throw "TM-DESKTOP-NOTIFICATIONS-VIEW: missing responsive Notifications contract $requiredPattern"
    }
}
foreach ($expiryVariant in @('ExactUtc', 'BoundedUtc', 'ProviderLocal', 'ProviderDate', 'Unknown')) {
    if ($uiRustText -notmatch "DesktopBenefitExpiry::$expiryVariant") {
        throw "TM-DESKTOP-NOTIFICATIONS-VIEW: missing expiry presentation $expiryVariant"
    }
}
$notificationsPublicText = @(
    [regex]::Match($notificationsProjectionText, '(?s)pub enum DesktopBenefitExpiry\s*\{.*?\r?\n\}').Value
    [regex]::Match($notificationsProjectionText, '(?s)pub struct DesktopReminderScopeRow\s*\{.*?\r?\n\}').Value
    [regex]::Match($notificationsProjectionText, '(?s)pub struct DesktopBenefitLotRow\s*\{.*?\r?\n\}').Value
    [regex]::Match($notificationsProjectionText, '(?s)pub struct DesktopNotificationsProjection\s*\{.*?\r?\n\}').Value
) -join "`n"
if ($notificationsPublicText -match '(?i)\b(?:provider|account|workspace|delivery|lot|scope|window|target|cursor|archive|credential|activation)[_-]?id\b|\b(?:absolute_)?path\b') {
    throw 'TM-DESKTOP-NOTIFICATIONS-IDENTITY: private identity or authority crossed the Notifications projection boundary'
}
$notificationsAdapterText = [regex]::Match(
    $uiRustText,
    '(?s)fn apply_notifications_projection\(.*?\r?\n\}\r?\n\r?\nfn notification_coverage_label'
).Value
if ([string]::IsNullOrWhiteSpace($notificationsAdapterText)) {
    throw 'TM-DESKTOP-NOTIFICATIONS-AUTHORITY: Notifications adapter boundary is absent'
}
$notificationsAuthorityBoundary = $notificationsProjectionText + "`n" +
    $notificationsViewText + "`n" + $notificationsAdapterText
$notificationsDeliveryPattern = '\b(?:take_notifications|acknowledge_notifications|release_notifications|BenefitReminderRuntime)\b'
$notificationsPollingPattern = '(?i)\b(?:poll_notifications|poll_reminders|Timer)\b'
$notificationsOwnerControlPattern = '(?i)\b(?:QueryService|UsageReadStore|UsageStore|Connection|rusqlite|VecDeque|HashMap|BTreeMap|LinkedList|sync_channel|notification_cache)\b|std::thread|thread::spawn|\bchannel\s*\(|callback\s+(?:activate|acknowledge|release|deliver|schedule)[A-Za-z0-9_-]*'
$notificationsDeliveryAuthorityCount = [regex]::Matches(
    $notificationsAuthorityBoundary,
    $notificationsDeliveryPattern
).Count
$notificationsPollingSurfaceCount = [regex]::Matches(
    $notificationsAuthorityBoundary,
    $notificationsPollingPattern
).Count
$notificationsOwnerControlCount = [regex]::Matches(
    $notificationsAuthorityBoundary,
    $notificationsOwnerControlPattern
).Count
if ($notificationsProjectionText -match '\.(?:opaque_id|target|delivery_id|lot_id|scope_id|account_id|workspace_id)\(' -or
    $notificationsDeliveryAuthorityCount -ne 0 -or
    $notificationsPollingSurfaceCount -ne 0 -or
    $notificationsOwnerControlCount -ne 0) {
    throw 'TM-DESKTOP-NOTIFICATIONS-AUTHORITY: Notifications route must remain read-only and delivery-receipt free'
}
if ($notificationsViewText -cnotmatch 'Text \{ text: scope\.completeness-label \+ " · " \+ scope\.evidence-label;[^\r\n]*visible: !root\.narrow;') {
    throw 'TM-DESKTOP-NOTIFICATIONS-VIEW: wide Notifications rows must preserve visible per-scope completeness'
}
$helpAboutViewPath = Join-Path $uiRoot 'views\help-about-view.slint'
$helpAboutViewText = [System.IO.File]::ReadAllText($helpAboutViewPath)
$helpAboutBoundary = $mainUiText + "`n" + $helpAboutViewText
if ($mainUiText -cnotmatch 'out property <string> help-about-layout-mode: help-view\.layout-mode;') {
    throw 'TM-DESKTOP-HELP-ABOUT-VIEW: MainWindow must expose the child content-width layout truth'
}
if ($mainUiText -cnotmatch 'out property <int> help-about-section-count: help-view\.section-count;') {
    throw 'TM-DESKTOP-HELP-ABOUT-BOUND: MainWindow must expose the child section-count truth'
}
foreach ($requiredPattern in @(
    'import \{ HelpAboutView \} from "views/help-about-view\.slint";',
    'out property <bool> help-about-visible: root\.active-route-key == "help_about";',
    'out property <string> help-about-layout-mode: help-view\.layout-mode;',
    'out property <int> help-about-section-count: help-view\.section-count;',
    'help-view := HelpAboutView',
    'visible: root\.help-about-visible;',
    '!root\.help-about-visible',
    'out property <bool> narrow: root\.width < 800px;',
    'out property <string> layout-mode: root\.narrow \? "narrow" : "wide";',
    'property <length> card-height: 232px;',
    'product-version: root\.help-product-version;'
)) {
    if ($helpAboutBoundary -cnotmatch $requiredPattern) {
        throw "TM-DESKTOP-HELP-ABOUT-VIEW: missing responsive Help About contract $requiredPattern"
    }
}
$helpAboutMountCount = [regex]::Matches(
    $mainUiText,
    '(?m)^\s*help-view := HelpAboutView\s*\{'
).Count
if ($helpAboutMountCount -ne 1 -or
    $mainUiText -match 'if root\.help-about-visible:\s*(?:[A-Za-z0-9_-]+\s*:=\s*)?HelpAboutView') {
    throw 'TM-DESKTOP-HELP-ABOUT-LIFECYCLE: Help About must stay mounted once and switch visibility only'
}
$helpAboutSectionCountMatch = [regex]::Match(
    $helpAboutViewText,
    'out property <int> section-count: ([0-9]+);'
)
$helpAboutSectionCount = if ($helpAboutSectionCountMatch.Success) {
    [int]$helpAboutSectionCountMatch.Groups[1].Value
} else {
    0
}
$helpAboutGuideCardCount = [regex]::Matches(
    $helpAboutViewText,
    '(?m)^\s*HelpSectionCard\s*\{'
).Count
$helpAboutAttributionCardCount = [regex]::Matches(
    $helpAboutViewText,
    '(?m)^\s*AttributionCard\s*\{'
).Count
$helpAboutRenderedSectionCount = $helpAboutGuideCardCount + $helpAboutAttributionCardCount
if ($helpAboutSectionCount -ne 6 -or
    $helpAboutGuideCardCount -ne 5 -or
    $helpAboutAttributionCardCount -ne 1 -or
    $helpAboutRenderedSectionCount -ne $helpAboutSectionCount -or
    [regex]::Matches($helpAboutViewText, 'out property <int> section-count:').Count -ne 1) {
    throw 'TM-DESKTOP-HELP-ABOUT-BOUND: Help About must expose exactly six fixed sections'
}
$helpAboutAttributionCount = [regex]::Matches($helpAboutViewText, '\bAboutSlint\s*\{').Count
$helpAboutAttributionImportCount = [regex]::Matches(
    $helpAboutViewText,
    'import \{ AboutSlint, ScrollView \} from "std-widgets\.slint";'
).Count
$helpAboutAttributionHeightCount = [regex]::Matches(
    $helpAboutViewText,
    'AboutSlint\s*\{\s*height: 112px;'
).Count
$helpAboutAttributionTextSizeCount = [regex]::Matches(
    $helpAboutViewText,
    '(?s)text: "WhereMyTokens and ccusage are pinned external MIT references, not runtime dependencies\.";\s*color:[^;]+;\s*font-size: 10px;'
).Count
if ($helpAboutAttributionCount -ne 1 -or
    $helpAboutAttributionImportCount -ne 1 -or
    $helpAboutAttributionHeightCount -ne 1 -or
    $helpAboutAttributionTextSizeCount -ne 1) {
    throw 'TM-DESKTOP-HELP-ABOUT-ATTRIBUTION: Help About must mount exactly one standard Slint attribution widget'
}
foreach ($requiredText in @(
    'Start here',
    'Data sources and truth',
    'Privacy by design',
    'Health and recovery',
    'Automation status',
    'About and licenses',
    'No prompts, responses, reasoning, commands',
    'CLI and stdio MCP are not available',
    'No browser session reuse or private endpoint replay',
    'Data Health owns backup, verification, restore, rebuild, and recovery truth. Settings owns backup policy and portable configuration.',
    'TokenMaster · MIT',
    $fixedUpstreamAttribution
)) {
    if (-not $helpAboutViewText.Contains($requiredText, [System.StringComparison]::Ordinal)) {
        throw "TM-DESKTOP-HELP-ABOUT-CONTENT: missing truthful Help About content $requiredText"
    }
}
$helpAboutAccessibleRegionCount = [regex]::Matches(
    $helpAboutViewText,
    'accessible-role:\s*region;'
).Count
if ($helpAboutAccessibleRegionCount -ne 4) {
    throw 'TM-DESKTOP-HELP-ABOUT-VIEW: Help About accessible region structure drifted'
}
$helpAboutVersionSetterPattern = 'set_help_product_version\(env!\("CARGO_PKG_VERSION"\)\.into\(\)\)'
$helpAboutVersionSetterCount = [regex]::Matches(
    $uiRustText,
    $helpAboutVersionSetterPattern
).Count
$helpAboutConstructor = [regex]::Match(
    $uiRustText,
    '(?s)pub fn new_with_reliable_state_and_session_sink\(.*?\r?\n    \}\r?\n\r?\n    #\[must_use\]'
).Value
if ($helpAboutVersionSetterCount -ne 1 -or
    [regex]::Matches($helpAboutConstructor, $helpAboutVersionSetterPattern).Count -ne 1 -or
    $uiRustText -match 'std::env::var|option_env!|git describe') {
    throw 'TM-DESKTOP-HELP-ABOUT-VERSION: Help About version must be applied exactly once from the compile-time package version'
}
$helpAboutModelPattern = '(?i)\b(?:ModelRc|VecModel|model\s*<)\b|property\s*<\[|(?m)^\s*for\s+[A-Za-z0-9_-]+\s+in\s+'
$helpAboutAuthorityPattern = '(?i)\bcallback\b|Platform\.open-url|https?://|\b(?:QueryService|UsageReadStore|UsageStore|Connection|rusqlite|reqwest|webbrowser)\b|std::(?:env|fs|net|process)|\b(?:activate|acknowledge|deliver|schedule)-benefit\b'
$helpAboutPollingPattern = '(?i)\b(?:Timer|poll_help|poll_about|thread::spawn|thread::sleep)\b'
$helpAboutModelCount = [regex]::Matches($helpAboutViewText, $helpAboutModelPattern).Count
$helpAboutAuthorityCount = [regex]::Matches($helpAboutViewText, $helpAboutAuthorityPattern).Count
$helpAboutPollingSurfaceCount = [regex]::Matches($helpAboutViewText, $helpAboutPollingPattern).Count
if ($helpAboutAuthorityCount -ne 0) {
    throw 'TM-DESKTOP-HELP-ABOUT-AUTHORITY: Help About must remain static and control-free'
}
if ($helpAboutModelCount -ne 0 -or $helpAboutPollingSurfaceCount -ne 0) {
    throw 'TM-DESKTOP-HELP-ABOUT-BOUND: Help About must not own models timers or polling'
}
$helpAboutFalseClaimPattern = '(?i)\b(?:release (?:accepted|ready|complete)|package (?:signed|ready)|signed (?:build|package|release)|SBOM (?:included|available|complete)|MSVC (?:build|release) (?:available|complete)|CLI is available|stdio MCP is available|automation is available|all providers (?:are )?(?:supported|available)|every provider (?:is )?(?:supported|available))\b'
if ($helpAboutViewText -match $helpAboutFalseClaimPattern) {
    throw 'TM-DESKTOP-HELP-ABOUT-CLAIM: Help About must not claim deferred release or automation capability'
}
$sessionsPath = Join-Path $sourceRoot 'sessions.rs'
$sessionsText = [System.IO.File]::ReadAllText($sessionsPath)
if ($sessionsText -notmatch 'pub const MAX_SESSION_ROWS: usize = 64;' -or
    $sessionsText -notmatch '\.take\(MAX_SESSION_ROWS\)') {
    throw 'TM-DESKTOP-SESSIONS-BOUND: sessions projection must retain at most sixty-four rows'
}
if ($controllerText -notmatch 'pub const MAX_SESSION_ROWS: usize = 64;' -or
    $controllerText -notmatch 'PageSize::new\(Self::MAX_SESSION_ROWS\)') {
    throw 'TM-DESKTOP-SESSIONS-REQUEST: sessions query must remain one bounded first page'
}
$sessionsProjectionCallCount = [regex]::Matches($uiRustText, 'apply_sessions_projection\(').Count
if ($sessionsProjectionCallCount -ne 2) {
    throw 'TM-DESKTOP-SESSIONS-REBUILD: sessions models must not rebuild during route-only selection'
}
$sessionsModelReplacementCount = [regex]::Matches(
    $uiRustText,
    'set_session_list_rows\(model\(rows\)\)'
).Count
if ($sessionsModelReplacementCount -ne 1) {
    throw 'TM-DESKTOP-SESSIONS-MODEL: sessions must have one bounded model replacement site'
}
$sessionDetailBounds = [ordered]@{
    MAX_SESSION_DETAIL_MODEL_ROWS = 32
    MAX_SESSION_DETAIL_PROJECT_ROWS = 32
}
foreach ($bound in $sessionDetailBounds.GetEnumerator()) {
    $pattern = "pub const $([regex]::Escape($bound.Key)): usize = $($bound.Value);"
    if ($sessionsText -notmatch $pattern) {
        throw "TM-DESKTOP-SESSION-DETAIL-BOUND: $($bound.Key) drifted"
    }
}
if ($sessionsText -notmatch 'Vec::with_capacity\(MAX_SESSION_DETAIL_MODEL_ROWS \+ MAX_SESSION_DETAIL_PROJECT_ROWS\)' -or
    $sessionsText -notmatch 'model_count >= MAX_SESSION_DETAIL_MODEL_ROWS' -or
    $sessionsText -notmatch 'project_count >= MAX_SESSION_DETAIL_PROJECT_ROWS') {
    throw 'TM-DESKTOP-SESSION-DETAIL-BOUND: exact session detail must retain at most 32 model and 32 project rows'
}
$sessionDetailQueuePattern = '(?im)^(?=[^\r\n]*(?:session|detail))[^\r\n]*(?:\b(?:VecDeque|HashMap|BTreeMap|LinkedList)\b|\bVec\s*<|\b(?:sync_)?channel\s*(?:::)?\s*(?:<|\())'
if ($controllerText -notmatch 'pending_selection:\s*Option<PendingDesktopSessionDetail>' -or
    $controllerText -notmatch 'latest_selection_generation:\s*Option<ProductSessionDetailSelectionGeneration>' -or
    $controllerText -match $sessionDetailQueuePattern) {
    throw 'TM-DESKTOP-SESSION-DETAIL-SLOT: exact detail work must use one latest-only typed slot'
}
$presentationText = [System.IO.File]::ReadAllText((Join-Path $sourceRoot 'presentation.rs'))
$sessionUiBoundaryText = $sessionsText + "`n" + $presentationText + "`n" + $uiRustText + "`n" + $uiText
if ($sessionUiBoundaryText -match '\bUsageSessionKey\b') {
    throw 'TM-DESKTOP-SESSION-DETAIL-IDENTITY: opaque session keys must remain inside the controller worker'
}
if ($controllerText -notmatch 'source\s*\.usage_session_detail\(key\)' -or
    $controllerText -notmatch 'DesktopSessionDetailIntent' -or
    $uiText -notmatch 'callback select-session\(int\)' -or
    $uiRustText -notmatch 'window\.on_select_session\(' -or
    $uiText -notmatch 'row-focus := FocusScope' -or
    $uiText -notmatch 'focus-on-tab-navigation:\s*true' -or
    $uiText -notmatch 'row-focus\.focus\(\)' -or
    $uiText -notmatch 'row-touch\.has-hover' -or
    $uiText -notmatch 'accessible-action-default') {
    throw 'TM-DESKTOP-SESSION-DETAIL-ROUTING: typed selection must route through the controller worker'
}
$sessionDetailModelReplacementCount = [regex]::Matches(
    $uiRustText,
    'set_session_detail_breakdown_rows\(model\(rows\)\)'
).Count
if ($sessionDetailModelReplacementCount -ne 1) {
    throw 'TM-DESKTOP-SESSION-DETAIL-MODEL: exact detail must have one bounded model replacement site'
}
$reliableStatePath = Join-Path $sourceRoot 'reliable_state.rs'
$reliableStateText = [System.IO.File]::ReadAllText($reliableStatePath)
if ($reliableStateText -notmatch 'pub const MAX_DESKTOP_RESTORE_POINTS: usize = 15;' -or
    $reliableStateText -notmatch '\.take\(MAX_DESKTOP_RESTORE_POINTS\)') {
    throw 'TM-DESKTOP-RESTORE-BOUND: reliable-state projection must retain at most fifteen points'
}
$restoreModelReplacementCount = [regex]::Matches($uiRustText, 'set_restore_point_rows\(model\(rows\)\)').Count
if ($restoreModelReplacementCount -ne 1) {
    throw 'TM-DESKTOP-RESTORE-MODEL: restore-point model must have one bounded replacement site'
}
if ($uiRustText -notmatch 'reviewed_restore_selection = Rc::new\(RefCell::new\(None\)\)' -or
    $uiRustText -notmatch 'reviewed_selection\.replace\(Some\(selection\)\)' -or
    $uiRustText -notmatch 'let selection = \*reviewed_selection\.borrow\(\)') {
    throw 'TM-DESKTOP-RESTORE-IDENTITY: confirmation must retain the exact reviewed generation and ordinal'
}
if ($reliableStateText -notmatch 'successful_count: Option<u64>' -or
    $reliableStateText -notmatch 'failure_count: Option<u64>' -or
    $reliableStateText -notmatch 'published_bytes: Option<u64>' -or
    [regex]::Matches($uiRustText, 'map_or_else\(\|\| "Unavailable"\.to_owned\(\)').Count -lt 3) {
    throw 'TM-DESKTOP-UNKNOWN-METRICS: unavailable metrics must remain typed unknowns in the UI'
}
foreach ($requiredIntent in @(
    'callback export-config\(\)',
    'callback import-config\(\)',
    'callback confirm-config-import\(\)',
    'callback cancel-config-import\(\)',
    'callback backup-normal\(\)',
    'callback backup-compact\(\)',
    'callback backup-encrypted\(string, string\)',
    'callback verify-backups\(\)',
    'callback preview-restore\(int\)',
    'callback confirm-restore\(int, bool\)',
    'callback rebuild-data\(\)',
    'callback retry-operation\(\)',
    'callback cancel-operation\(\)',
    'callback update-backup-policy\(bool, int, int, int\)'
)) {
    if ($uiText -notmatch $requiredIntent) {
        throw "TM-DESKTOP-RELIABLE-INTENT: missing typed intent $requiredIntent"
    }
}
if ($uiText -notmatch 'passphrase\.text\s*=\s*""' -or
    $uiText -notmatch 'confirmation\.text\s*=\s*""') {
    throw 'TM-DESKTOP-SECRET-CLEAR: transient passphrase fields must clear after admission'
}
foreach ($requiredPolicyBound in @(
    'minimum:\s*300;\s*maximum:\s*3600',
    'minimum:\s*21600;\s*maximum:\s*604800',
    'minimum:\s*256;\s*maximum:\s*65536'
)) {
    if ($uiText -notmatch $requiredPolicyBound) {
        throw "TM-DESKTOP-POLICY-BOUND: backup policy control drifted: $requiredPolicyBound"
    }
}
$settingsViewText = [System.IO.File]::ReadAllText((Join-Path $uiRoot 'views\settings-view.slint'))
$reminderLeadCapCount = [regex]::Matches(
    $reliableStateText,
    'pub const MAX_DESKTOP_REMINDER_LEADS: usize = 8;'
).Count
$reminderPresetCount = [regex]::Matches(
    $mainUiText,
    'in-out property <bool> reminder-preset-(?:seven-days|twenty-four-hours|twelve-hours|six-hours|one-hour): false;'
).Count
$allReminderPresetCount = [regex]::Matches(
    $mainUiText,
    'in-out property <bool> reminder-preset-[A-Za-z0-9-]+: false;'
).Count
if ($reminderLeadCapCount -ne 1 -or $reminderPresetCount -ne 5 -or $allReminderPresetCount -ne 5) {
    throw 'TM-DESKTOP-REMINDER-BOUND: reminder leads must remain capped at eight with exactly five presets'
}
$reminderRowsFunction = [regex]::Match(
    $uiRustText,
    '(?s)fn reminder_custom_rows\(.*?\r?\n\}\r?\n\r?\nfn apply_route_projection'
).Value
$reminderRowModelCount = [regex]::Matches(
    $mainUiText,
    'in-out property <\[ReminderCustomLeadRow\]> reminder-custom-lead-rows;'
).Count
$reminderRowUpdateCount = [regex]::Matches($uiRustText, 'rows\.set_row_data\(').Count
if ([string]::IsNullOrWhiteSpace($reminderRowsFunction) -or $reminderRowModelCount -ne 1 -or
    $reminderRowUpdateCount -ne 1 -or $reminderRowsFunction -notmatch 'Vec::with_capacity\(8\)' -or
    $reminderRowsFunction -notmatch '\.take\(8\)' -or
    $reminderRowsFunction -notmatch 'rows\.resize\(\s*8,') {
    throw 'TM-DESKTOP-REMINDER-MODEL: one fixed eight-row reminder model and one row update site are required'
}
$reminderIntentBindingCount = [regex]::Matches(
    $uiRustText,
    '(?s)window\.on_save_reminder_policy\(move \|\| \{.*?DesktopIntent::update_reminder_policy\('
).Count
if ($mainUiText -notmatch 'callback reminder-custom-lead-edited\(int, bool, int, int\);' -or
    $mainUiText -notmatch 'root\.reminder-custom-lead-edited\(index, enabled, value, unit-index\);' -or
    $settingsViewText -notmatch 'callback reminder-custom-lead-edited\(int, bool, int, int\);' -or
    $reminderIntentBindingCount -ne 1) {
    throw 'TM-DESKTOP-REMINDER-ROUTING: root forwarding and one checked typed reminder intent are required'
}
$reminderProjectionFunction = [regex]::Match(
    $uiRustText,
    '(?s)fn reminder_custom_rows\(.*?\r?\n\}\r?\n\r?\nfn apply_route_projection'
).Value
if ($reminderProjectionFunction -notmatch 'is_multiple_of\(86_400\)' -or
    $reminderProjectionFunction -notmatch 'is_multiple_of\(3_600\)' -or
    $reminderProjectionFunction -notmatch 'is_multiple_of\(60\)' -or
    $uiRustText -notmatch '\.filter\(\|lead\| !\[604_800, 86_400, 43_200, 21_600, 3_600\]\.contains\(lead\)\)') {
    throw 'TM-DESKTOP-REMINDER-PROJECTION: custom leads must use the largest exact unit and exclude presets'
}
$reminderSaveFunction = [regex]::Match(
    $uiRustText,
    '(?s)window\.on_save_reminder_policy\(move \|\| \{.*?\n    \}\);'
).Value
$reliableDeliveryFunction = [regex]::Match(
    $uiRustProductionText,
    '(?s)fn deliver_latest\(.*?\r?\n    \}\r?\n\}'
).Value
$pendingApplyIndex = $reliableDeliveryFunction.IndexOf('apply_reliable_state_projection(&window, &delivery.projection);', [System.StringComparison]::Ordinal)
$pendingAckIndex = $reliableDeliveryFunction.IndexOf('acknowledgement.send(if delivered', [System.StringComparison]::Ordinal)
if ($uiRustProductionText -notmatch 'const VISIBLE_REMINDER_PUBLICATION_TIMEOUT: Duration = Duration::from_secs\(5\);' -or
    [regex]::Matches($uiRustProductionText, 'recv_timeout\(VISIBLE_REMINDER_PUBLICATION_TIMEOUT\)').Count -ne 1 -or
    [string]::IsNullOrWhiteSpace($reliableDeliveryFunction) -or $pendingApplyIndex -lt 0 -or
    $pendingAckIndex -le $pendingApplyIndex) {
    throw 'TM-DESKTOP-REMINDER-VISIBLE-PENDING: bounded acknowledgement must follow visible Pending projection application'
}
$reminderVisiblePendingAckCount = 1
$reliableProjectionFunction = [regex]::Match(
    $uiRustText,
    '(?s)fn apply_reliable_state_projection\(.*?\r?\n\}\r?\n\r?\nfn saturating_i32'
).Value
$reminderSyncStateIndex = $reliableProjectionFunction.IndexOf('window.set_reminder_sync_state(', [System.StringComparison]::Ordinal)
$reminderDirtyIndex = $reliableProjectionFunction.IndexOf('if !window.get_reminder_dirty() {', [System.StringComparison]::Ordinal)
$reminderRejectedIndex = $reminderSaveFunction.IndexOf('DesktopIntentAdmission::Rejected =>', [System.StringComparison]::Ordinal)
$reminderAcceptedIndex = $reminderSaveFunction.IndexOf('DesktopIntentAdmission::Started', [System.StringComparison]::Ordinal)
$reminderClearIndex = $reminderSaveFunction.IndexOf('window.set_reminder_dirty(false);', [System.StringComparison]::Ordinal)
if ([string]::IsNullOrWhiteSpace($reminderSaveFunction) -or $reminderRejectedIndex -lt 0 -or
    $reminderAcceptedIndex -le $reminderRejectedIndex -or $reminderClearIndex -le $reminderAcceptedIndex -or
    [string]::IsNullOrWhiteSpace($reliableProjectionFunction) -or $reminderSyncStateIndex -lt 0 -or
    $reminderDirtyIndex -le $reminderSyncStateIndex) {
    throw 'TM-DESKTOP-REMINDER-DRAFT: sync truth must update while dirty drafts persist and rejected saves retain drafts'
}
if ([regex]::Matches($uiRustText, 'let lead = value\.checked_mul\(unit\)\?;').Count -ne 1) {
    throw 'TM-DESKTOP-REMINDER-CONVERSION: custom lead conversion must use one checked multiplication'
}
foreach ($accessibleReminderLabel in @(
    'Reminder synchronization state ',
    'Enable expiry reminders',
    'Reminder lead time 7 days',
    'Enable custom reminder lead row ',
    'Custom reminder lead value row ',
    'Custom reminder lead unit row ',
    'Save reminder profile',
    'Reset reminder profile to recommended'
)) {
    if ($settingsViewText -notmatch [regex]::Escape($accessibleReminderLabel)) {
        throw 'TM-DESKTOP-REMINDER-ACCESSIBILITY: reminder controls require distinct stable accessible labels'
    }
}
$reminderScrollCount = [regex]::Matches($settingsViewText, 'settings-scroll := ScrollView \{').Count
$reminderCardCount = [regex]::Matches($settingsViewText, 'reminder-card := Rectangle \{').Count
$backupCardCount = [regex]::Matches($settingsViewText, 'backup-card := Rectangle \{').Count
$backupSaveCount = [regex]::Matches($settingsViewText, 'text: "Save backup policy"').Count
if ($reminderScrollCount -ne 1 -or $reminderCardCount -ne 1 -or $backupCardCount -ne 1 -or
    $backupSaveCount -ne 1 -or $settingsViewText -notmatch 'out property <length> reminder-card-bottom:' -or
    $settingsViewText -notmatch 'out property <length> backup-card-top:') {
    throw 'TM-DESKTOP-REMINDER-LAYOUT: one intrinsic ScrollView reminder card and one responsive backup control set are required'
}
$reminderOwnerBoundary = $mainUiText + "`n" + $settingsViewText + "`n" + $uiRustProductionText
$reminderOwnerRemainder = $reminderOwnerBoundary -replace 'sync_channel\(1\)', '' -replace '\bmpsc::\{SyncSender, sync_channel\}', ''
if ($reminderOwnerRemainder -match '\b(?:Timer|VecDeque|sync_channel|thread::spawn|thread::sleep|animate|LineEdit)\b') {
    throw 'TM-DESKTOP-REMINDER-OWNER: reminder settings must not add a timer, animation, parser, worker, polling loop, or queue'
}
$dashboardModelReplacementCount = [regex]::Matches(
    $uiRustText,
    'set_dashboard_(?:section_rows|quota_rows|benefit_rows|trend_points|session_rows|activity_rows|model_rows)\(model\('
).Count
if ($dashboardModelReplacementCount -ne 7) {
    throw 'TM-DESKTOP-DASHBOARD-MODEL: dashboard must replace each of seven bounded list models exactly once'
}

$presentationPath = Join-Path $sourceRoot 'presentation.rs'
$presentationText = [System.IO.File]::ReadAllText($presentationPath)
$stableStart = $presentationText.IndexOf('pub const fn stable_key', [System.StringComparison]::Ordinal)
$stableEnd = $presentationText.IndexOf('pub const fn label_key', [System.StringComparison]::Ordinal)
if ($stableStart -lt 0 -or $stableEnd -le $stableStart) {
    throw 'TM-DESKTOP-ROUTE-COUNT: stable route-key boundary is absent'
}
$stableSection = $presentationText.Substring($stableStart, $stableEnd - $stableStart)
$expectedRouteKeys = @(
    'dashboard', 'history', 'sessions', 'models', 'projects', 'activity',
    'data_health', 'notifications', 'settings', 'help_about', 'compact_widget'
)
$routeMatches = [regex]::Matches($stableSection, 'Self::[A-Za-z]+\s*=>\s*"([a-z_]+)"')
$actualRouteKeys = @($routeMatches | ForEach-Object { $_.Groups[1].Value })
if (
    $actualRouteKeys.Count -ne 11 -or
    @($expectedRouteKeys | Where-Object { $_ -notin $actualRouteKeys }).Count -ne 0 -or
    @($actualRouteKeys | Sort-Object -Unique).Count -ne 11
) {
    throw 'TM-DESKTOP-ROUTE-COUNT: desktop route keys drifted from the fixed 11-route contract'
}

foreach ($requiredPattern in @(
    'pub const DESKTOP_ROUTE_COUNT: usize = ProductRoute::ALL\.len\(\)',
    'values: \[Option<&''static str>; MAX_ROUTE_REASONS\]',
    'const MAX_ROUTE_REASONS: usize = 11',
    'DesktopApplyOutcome::IgnoredNotNewer',
    'std::array::from_fn',
    'ModelRc::new\(VecModel::from\(rows\)\)',
    'ProductReducer::new\(\)',
    'reducer\.snapshot\(\)',
    'winit-software'
)) {
    if ($productionText -notmatch $requiredPattern) {
        throw "TM-DESKTOP-MISSING-CONTRACT: $requiredPattern"
    }
}
if ($productionText -match '\b(QuotaRow|SessionRow|ChartPoint|quota-targets|chart-points)\b') {
    throw 'TM-DESKTOP-MOCK-DATA: production shell contains probe data models'
}

if ($SourceOnly) {
    [ordered]@{
        result = 'pass'
        scope = 'source-only'
        fixed_route_count = 11
        rust_source_file_count = $rustFiles.Count
        slint_source_file_count = $uiFiles.Count
        density_variant_count = $densityVariantCount
        density_stable_key_arm_count = $stableKeyArmCount
        density_slint_index_arm_count = $slintIndexArmCount
        density_from_slint_index_arm_count = $fromSlintIndexArmCount
        density_token_table_count = $densityTokenDeclarationCount
        density_owner_count = $presentationStyleOwnerCount
        density_owner_slot_count = $presentationStyleOwnerSlotCount
        density_root_binding_count = $rootDensityBindingCount
        density_root_callback_count = $rootDensityCallbackCount
        density_wiring_callback_count = $densityWiringCallbackCount
        density_revision_type_count = $presentationRevisionTypeCount
        density_checked_successor_count = $checkedSuccessorDerivationCount
        density_successor_call_count = $checkedSuccessorCallCount
        density_write_count = $densityWriteCount
        density_revision_write_count = $revisionWriteCount
        density_switch_loop_count = $densitySwitchLoopCount
        density_applied_assertion_count = $densityAppliedAssertionCount
        density_final_postcondition_count = $densityFinalPostconditionCount
        density_authority_count = $densityAuthorityCount
        density_allowed_owner_occurrence_count = $densityAllowedOwnerOccurrenceCount
        density_allowed_owner_wire_signature_count = $densityAllowedOwnerWireSignatureCount
        density_authority_timer_delay_interval_sleep_count = $densityAuthorityCategoryCounts.timer_delay_interval_sleep
        density_authority_worker_thread_spawn_task_count = $densityAuthorityCategoryCounts.worker_thread_spawn_task
        density_authority_query_count = $densityAuthorityCategoryCounts.query
        density_authority_window_create_count = $densityAuthorityCategoryCounts.window_create
        density_authority_queue_deque_count = $densityAuthorityCategoryCounts.queue_deque
        density_authority_cache_count = $densityAuthorityCategoryCounts.cache
        density_authority_channel_count = $densityAuthorityCategoryCounts.channel
        density_authority_unsafe_count = $densityAuthorityCategoryCounts.unsafe
        density_authority_retained_count = $densityAuthorityCategoryCounts.retained
        skin_variant_count = $skinVariantCount
        skin_key_mapping_count = $skinKeyMappingCount
        skin_index_mapping_count = $skinIndexMappingCount
        skin_reverse_index_mapping_count = $skinReverseIndexMappingCount
        palette_role_count = $paletteRoles.Count
        palette_exact_rgb_value_count = $paletteRgbValueCount
        palette_slot_count = $skinRootBindingCount
        skin_root_callback_count = $skinRootCallbackCount
        skin_settings_callback_count = $settingsSkinCallbackCount
        skin_forward_binding_count = $skinForwardBindingCount
        skin_wiring_callback_count = $skinWiringCallbackCount
        palette_order_count = [int]($paletteIndex -ge 0 -and $metadataIndex -gt $paletteIndex)
        command_palette_query_scalar_maximum = $commandPaletteQueryCap
        command_palette_model_count = $commandPaletteModelCount
        command_palette_shortcut_count = $commandPaletteShortcutCount
        command_palette_accessible_default_action_count = $commandPaletteDefaultActionCount
        command_palette_route_only = $commandPaletteRouteOnly
        command_palette_owner_count = $commandPaletteOwnerCount
        compact_widget_quota_row_maximum = $dashboardBounds.MAX_DASHBOARD_QUOTA_ROWS
        compact_widget_quota_model_count = $compactWidgetQuotaPropertyCount
        compact_widget_geometry_slot_count = $compactWidgetGeometrySlotCount
        compact_widget_owner_count = $compactWidgetOwnerCount
        tray_component_count = $trayComponentCount
        tray_intent_count = $trayIntentCount
        tray_router_slot_count = $trayRouterSlotCount
        tray_close_handler_count = $trayCloseHandlerCount
        tray_owner_count = $trayOwnerCount
        tray_explorer_recovery_count = $trayExplorerRecoveryCount
        tray_readd_check_count = $trayReAddCheckCount
        tray_callback_binding_count = $trayCallbackBindingCount
        tray_focus_count = $trayFocusCount
        tray_polling_surface_count = $trayPollingSurfaceCount
        tray_icon_sha256 = $trayIconHash
        controller_worker_count = $workerConstructionCount
        retained_snapshot_slot_count = $snapshotSlotCount
        event_loop_schedule_site_count = $eventScheduleCount
        bridge_event_loop_schedule_site_count = $bridgeEventScheduleCount
        reliable_event_loop_schedule_site_count = $reliableEventScheduleCount
        bridge_polling_surface_count = 0
        dashboard_section_count = $dashboardBounds.DESKTOP_DASHBOARD_SECTION_COUNT
        dashboard_model_replacement_count = $dashboardModelReplacementCount
        dashboard_projection_application_count = $dashboardProjectionCallCount - 1
        dashboard_polling_surface_count = 0
        history_day_maximum = 30
        history_model_replacement_count = $historyModelReplacementCount
        history_projection_application_count = $historyProjectionCallCount - 1
        history_polling_surface_count = 0
        model_row_maximum = 64
        models_model_replacement_count = $modelsModelReplacementCount
        models_projection_application_count = $modelsProjectionCallCount - 1
        analytics_query_call_count = $analyticsQueryCallCount
        models_polling_surface_count = 0
        project_row_maximum = 32
        projects_model_replacement_count = $projectsModelReplacementCount
        projects_projection_application_count = $projectsProjectionCallCount - 1
        git_query_call_count = $gitQueryCallCount
        projects_polling_surface_count = 0
        activity_row_maximum = 12
        activity_model_replacement_count = $activityModelReplacementCount
        activity_projection_application_count = $activityProjectionCallCount - 1
        activity_query_call_count = $activityQueryCallCount
        activity_polling_surface_count = 0
        notification_scope_maximum = $notificationBounds.MAX_NOTIFICATION_SCOPES
        notification_lot_maximum = $notificationBounds.MAX_NOTIFICATION_LOTS
        notification_lead_maximum = $notificationBounds.MAX_NOTIFICATION_LEADS
        notification_scope_model_replacement_count = $notificationScopeModelReplacementCount
        notification_lot_model_replacement_count = $notificationLotModelReplacementCount
        notifications_projection_application_count = $notificationsProjectionCallCount - 1
        benefit_query_call_count = $benefitQueryCallCount
        notifications_delivery_authority_count = $notificationsDeliveryAuthorityCount
        notifications_owner_control_count = $notificationsOwnerControlCount
        notifications_polling_surface_count = $notificationsPollingSurfaceCount
        in_app_notification_row_maximum = 256
        in_app_notification_model_count = $inAppModelCount
        in_app_notification_presented_after_apply_count = $successfulApplyCount
        in_app_notification_ready_before_receipt_count = $readyBeforeReceiptCount
        in_app_notification_accessible_label_count = 1
        in_app_notification_epoch_guard_count = $inAppEpochGuardCount
        reminder_lead_cap = 8
        reminder_preset_count = $reminderPresetCount
        reminder_row_model_count = $reminderRowModelCount
        reminder_row_update_count = $reminderRowUpdateCount
        reminder_checked_conversion_count = 1
        reminder_accessible_label_count = 8
        reminder_scrollview_count = $reminderScrollCount
        reminder_visible_pending_ack_count = $reminderVisiblePendingAckCount
        reminder_visible_pending_channel_count = $reliableAckChannelCount
        help_about_section_count = $helpAboutRenderedSectionCount
        help_about_version_setter_count = $helpAboutVersionSetterCount
        help_about_slint_attribution_count = $helpAboutAttributionCount
        help_about_model_count = $helpAboutModelCount
        help_about_authority_count = $helpAboutAuthorityCount
        help_about_polling_surface_count = $helpAboutPollingSurfaceCount
        session_row_maximum = 64
        session_detail_model_row_maximum = 32
        session_detail_project_row_maximum = 32
        sessions_model_replacement_count = $sessionsModelReplacementCount
        session_detail_model_replacement_count = $sessionDetailModelReplacementCount
        sessions_projection_application_count = $sessionsProjectionCallCount - 1
        sessions_polling_surface_count = 0
        restore_point_maximum = 15
        restore_model_replacement_count = $restoreModelReplacementCount
        secret_model_count = 0
        private_ui_identity_count = 0
    } | ConvertTo-Json -Compress
    return
}

$metadataJson = & cargo +1.97.0 metadata --locked --format-version 1 --manifest-path $rootManifest
if ($LASTEXITCODE -ne 0) {
    throw 'TM-DESKTOP-METADATA: cargo metadata failed'
}
$metadata = $metadataJson | ConvertFrom-Json -Depth 100
$desktopPackages = @($metadata.packages | Where-Object { $_.name -eq 'tokenmaster-desktop' })
if ($desktopPackages.Count -ne 1) {
    throw 'TM-DESKTOP-PACKAGE: tokenmaster-desktop must resolve exactly once'
}
$directProductionDependencies = @(
    $desktopPackages[0].dependencies |
        Where-Object { $null -eq $_.kind } |
        ForEach-Object { $_.name } |
        Sort-Object -Unique
)
$expectedDependencies = @(
    'anyhow', 'chrono', 'slint', 'tokenmaster-domain', 'tokenmaster-engine',
    'tokenmaster-product', 'tokenmaster-query', 'raw-window-handle', 'windows'
)
if (
    $directProductionDependencies.Count -ne $expectedDependencies.Count -or
    @($expectedDependencies | Where-Object { $_ -notin $directProductionDependencies }).Count -ne 0
) {
    throw "TM-DESKTOP-DIRECT-AUTHORITY: direct dependency set drifted: $($directProductionDependencies -join ', ')"
}

$featureTree = (& cargo +1.97.0 tree -p tokenmaster-desktop -e features --manifest-path $rootManifest) -join "`n"
if ($LASTEXITCODE -ne 0) {
    throw 'TM-DESKTOP-TREE: cargo feature tree failed'
}
if ($featureTree -notmatch 'renderer-software') {
    throw 'TM-DESKTOP-SOFTWARE-RENDERER: software renderer is absent'
}
if ($featureTree -match 'renderer-femtovg') {
    throw 'TM-DESKTOP-FEMTOVG: package feature tree contains FemtoVG'
}
if ($featureTree -match 'tokenmaster-m0') {
    throw 'TM-DESKTOP-PROBE-DEPENDENCY: package tree contains the M0 probe'
}

& cargo +1.97.0 build --release --locked --manifest-path $rootManifest -p tokenmaster-desktop
if ($LASTEXITCODE -ne 0) {
    throw 'TM-DESKTOP-BUILD: release desktop build failed'
}

[ordered]@{
    result = 'pass'
    package = 'tokenmaster-desktop'
    binary = $null
    direct_production_dependencies = $directProductionDependencies
    rust_source_file_count = $rustFiles.Count
    slint_source_file_count = $uiFiles.Count
    density_variant_count = $densityVariantCount
    density_stable_key_arm_count = $stableKeyArmCount
    density_slint_index_arm_count = $slintIndexArmCount
    density_from_slint_index_arm_count = $fromSlintIndexArmCount
    density_token_table_count = $densityTokenDeclarationCount
    density_owner_count = $presentationStyleOwnerCount
    density_owner_slot_count = $presentationStyleOwnerSlotCount
    density_root_binding_count = $rootDensityBindingCount
    density_root_callback_count = $rootDensityCallbackCount
    density_wiring_callback_count = $densityWiringCallbackCount
    density_revision_type_count = $presentationRevisionTypeCount
    density_checked_successor_count = $checkedSuccessorDerivationCount
    density_successor_call_count = $checkedSuccessorCallCount
    density_write_count = $densityWriteCount
    density_revision_write_count = $revisionWriteCount
    density_switch_loop_count = $densitySwitchLoopCount
    density_applied_assertion_count = $densityAppliedAssertionCount
    density_final_postcondition_count = $densityFinalPostconditionCount
    density_authority_count = $densityAuthorityCount
    density_allowed_owner_occurrence_count = $densityAllowedOwnerOccurrenceCount
    density_allowed_owner_wire_signature_count = $densityAllowedOwnerWireSignatureCount
    density_authority_timer_delay_interval_sleep_count = $densityAuthorityCategoryCounts.timer_delay_interval_sleep
    density_authority_worker_thread_spawn_task_count = $densityAuthorityCategoryCounts.worker_thread_spawn_task
    density_authority_query_count = $densityAuthorityCategoryCounts.query
    density_authority_window_create_count = $densityAuthorityCategoryCounts.window_create
    density_authority_queue_deque_count = $densityAuthorityCategoryCounts.queue_deque
    density_authority_cache_count = $densityAuthorityCategoryCounts.cache
    density_authority_channel_count = $densityAuthorityCategoryCounts.channel
    density_authority_unsafe_count = $densityAuthorityCategoryCounts.unsafe
    density_authority_retained_count = $densityAuthorityCategoryCounts.retained
    skin_variant_count = $skinVariantCount
    skin_key_mapping_count = $skinKeyMappingCount
    skin_index_mapping_count = $skinIndexMappingCount
    skin_reverse_index_mapping_count = $skinReverseIndexMappingCount
    palette_role_count = $paletteRoles.Count
    palette_exact_rgb_value_count = $paletteRgbValueCount
    palette_slot_count = $skinRootBindingCount
    skin_root_callback_count = $skinRootCallbackCount
    skin_settings_callback_count = $settingsSkinCallbackCount
    skin_forward_binding_count = $skinForwardBindingCount
    skin_wiring_callback_count = $skinWiringCallbackCount
    palette_order_count = [int]($paletteIndex -ge 0 -and $metadataIndex -gt $paletteIndex)
    command_palette_query_scalar_maximum = $commandPaletteQueryCap
    command_palette_model_count = $commandPaletteModelCount
    command_palette_shortcut_count = $commandPaletteShortcutCount
    command_palette_accessible_default_action_count = $commandPaletteDefaultActionCount
    command_palette_route_only = $commandPaletteRouteOnly
    command_palette_owner_count = $commandPaletteOwnerCount
    compact_widget_quota_row_maximum = $dashboardBounds.MAX_DASHBOARD_QUOTA_ROWS
    compact_widget_quota_model_count = $compactWidgetQuotaPropertyCount
    compact_widget_geometry_slot_count = $compactWidgetGeometrySlotCount
    compact_widget_owner_count = $compactWidgetOwnerCount
    tray_component_count = $trayComponentCount
    tray_intent_count = $trayIntentCount
    tray_router_slot_count = $trayRouterSlotCount
    tray_close_handler_count = $trayCloseHandlerCount
    tray_owner_count = $trayOwnerCount
    tray_explorer_recovery_count = $trayExplorerRecoveryCount
    tray_readd_check_count = $trayReAddCheckCount
    tray_callback_binding_count = $trayCallbackBindingCount
    tray_focus_count = $trayFocusCount
    tray_polling_surface_count = $trayPollingSurfaceCount
    tray_icon_sha256 = $trayIconHash
    fixed_route_count = 11
    maximum_route_reason_count = 11
    retained_route_model_count = 1
    controller_worker_count = $workerConstructionCount
    retained_snapshot_slot_count = $snapshotSlotCount
    event_loop_schedule_site_count = $eventScheduleCount
    bridge_event_loop_schedule_site_count = $bridgeEventScheduleCount
    reliable_event_loop_schedule_site_count = $reliableEventScheduleCount
    bridge_polling_surface_count = 0
    dashboard_section_count = $dashboardBounds.DESKTOP_DASHBOARD_SECTION_COUNT
    dashboard_model_replacement_count = $dashboardModelReplacementCount
    dashboard_projection_application_count = $dashboardProjectionCallCount - 1
    dashboard_polling_surface_count = 0
    history_day_maximum = 30
    history_model_replacement_count = $historyModelReplacementCount
    history_projection_application_count = $historyProjectionCallCount - 1
    history_polling_surface_count = 0
    model_row_maximum = 64
    models_model_replacement_count = $modelsModelReplacementCount
    models_projection_application_count = $modelsProjectionCallCount - 1
    analytics_query_call_count = $analyticsQueryCallCount
    models_polling_surface_count = 0
    project_row_maximum = 32
    projects_model_replacement_count = $projectsModelReplacementCount
    projects_projection_application_count = $projectsProjectionCallCount - 1
    git_query_call_count = $gitQueryCallCount
    projects_polling_surface_count = 0
    activity_row_maximum = 12
    activity_model_replacement_count = $activityModelReplacementCount
    activity_projection_application_count = $activityProjectionCallCount - 1
    activity_query_call_count = $activityQueryCallCount
    activity_polling_surface_count = 0
    notification_scope_maximum = $notificationBounds.MAX_NOTIFICATION_SCOPES
    notification_lot_maximum = $notificationBounds.MAX_NOTIFICATION_LOTS
    notification_lead_maximum = $notificationBounds.MAX_NOTIFICATION_LEADS
    notification_scope_model_replacement_count = $notificationScopeModelReplacementCount
    notification_lot_model_replacement_count = $notificationLotModelReplacementCount
    notifications_projection_application_count = $notificationsProjectionCallCount - 1
    benefit_query_call_count = $benefitQueryCallCount
    notifications_delivery_authority_count = $notificationsDeliveryAuthorityCount
    notifications_owner_control_count = $notificationsOwnerControlCount
    notifications_polling_surface_count = $notificationsPollingSurfaceCount
    in_app_notification_row_maximum = 256
    in_app_notification_model_count = $inAppModelCount
    in_app_notification_presented_after_apply_count = $successfulApplyCount
    in_app_notification_ready_before_receipt_count = $readyBeforeReceiptCount
    in_app_notification_accessible_label_count = 1
    in_app_notification_epoch_guard_count = $inAppEpochGuardCount
    reminder_lead_cap = 8
    reminder_preset_count = $reminderPresetCount
    reminder_row_model_count = $reminderRowModelCount
    reminder_row_update_count = $reminderRowUpdateCount
    reminder_checked_conversion_count = 1
    reminder_accessible_label_count = 8
    reminder_scrollview_count = $reminderScrollCount
    reminder_visible_pending_ack_count = $reminderVisiblePendingAckCount
    reminder_visible_pending_channel_count = $reliableAckChannelCount
    help_about_section_count = $helpAboutRenderedSectionCount
    help_about_version_setter_count = $helpAboutVersionSetterCount
    help_about_slint_attribution_count = $helpAboutAttributionCount
    help_about_model_count = $helpAboutModelCount
    help_about_authority_count = $helpAboutAuthorityCount
    help_about_polling_surface_count = $helpAboutPollingSurfaceCount
    session_row_maximum = 64
    session_detail_model_row_maximum = 32
    session_detail_project_row_maximum = 32
    sessions_model_replacement_count = $sessionsModelReplacementCount
    session_detail_model_replacement_count = $sessionDetailModelReplacementCount
    sessions_projection_application_count = $sessionsProjectionCallCount - 1
    sessions_polling_surface_count = 0
    restore_point_maximum = 15
    restore_model_replacement_count = $restoreModelReplacementCount
    secret_model_count = 0
    private_ui_identity_count = 0
    mock_data_model_count = 0
    direct_authority_dependency_count = 0
    forbidden_source_authority_count = 0
    femtovg_feature_count = 0
    probe_dependency_count = 0
    release_artifact_count = 0
} | ConvertTo-Json -Compress
