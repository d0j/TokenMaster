# P4 acceptance: the stranger test.
#
# A machine that has never built this repository extracts the shipped ZIP, runs it, and sees
# its own Codex history. This script is that reader: it extracts somewhere nothing was built,
# starts the executable from there, records how long the window took, lists every runtime
# library the process resolved so a dependency taken from System32 is visible, and reads the
# shell header and the Today card out of the accessibility tree at four moments while the
# first import runs.
#
# It declares DPI awareness before measuring anything. Without that, GetWindowRect returns
# virtualised coordinates on a scaled display and a capture drawn into a bitmap of that size
# silently loses the right and bottom edges -- three interface defects were once recorded and
# withdrawn because of exactly that.
#
# Operator-run, like `validate-p3e-interactive.ps1`. Nothing invokes it automatically.
param(
    [string]$Zip = "C:\code\tokenmaster\dist\TokenMaster-0.1.0-windows-x64-unsigned.zip"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$sig = @'
[DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
'@
$u = (Add-Type -MemberDefinition $sig -Name Stranger -Namespace Probe -PassThru) |
    Where-Object { $_.Name -eq 'Stranger' }
$u::SetProcessDpiAwarenessContext([IntPtr]-4) | Out-Null
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes

Get-Process -Name TokenMaster -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 500

$root = Join-Path $env:TEMP ("tokenmaster-stranger-" + [System.IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $root -Force | Out-Null
Expand-Archive -LiteralPath $Zip -DestinationPath $root -Force
"extracted to $root"

$exe = Get-ChildItem -LiteralPath $root -Recurse -File -Filter "TokenMaster.exe" |
    Select-Object -First 1
if (-not $exe) { throw "no TokenMaster.exe inside the package" }
"executable: {0} ({1:N1} MB)" -f $exe.FullName, ($exe.Length / 1MB)
"package contents:"
Get-ChildItem -LiteralPath $exe.Directory.FullName -File |
    ForEach-Object { "  {0,-28} {1,10:N0} bytes" -f $_.Name, $_.Length }

$started = Get-Date
$p = Start-Process -FilePath $exe.FullName -PassThru
$handle = [IntPtr]::Zero
while (((Get-Date) - $started).TotalSeconds -lt 60) {
    Start-Sleep -Milliseconds 400
    $live = Get-Process -Id $p.Id -ErrorAction SilentlyContinue
    if (-not $live) { throw "the packaged executable exited before showing a window" }
    if ($live.MainWindowHandle -ne 0) { $handle = $live.MainWindowHandle; break }
}
if ($handle -eq [IntPtr]::Zero) { throw "no window within 60 seconds" }
"window appeared after {0:N1} s" -f ((Get-Date) - $started).TotalSeconds

# every DLL the process resolved, so a dependency taken from System32 is visible
Start-Sleep -Seconds 20
$live = Get-Process -Id $p.Id
$fromPackage = @()
$fromSystem = @()
foreach ($m in $live.Modules) {
    if ($m.ModuleName -match '^(VCRUNTIME|MSVCP)') {
        if ($m.FileName -like "$root*") { $fromPackage += $m.ModuleName }
        else { $fromSystem += "$($m.ModuleName) <- $($m.FileName)" }
    }
}
"runtime libraries from the package: {0}" -f ($fromPackage -join ", ")
if ($fromSystem.Count) { "runtime libraries from elsewhere:"; $fromSystem }

$automation = [System.Windows.Automation.AutomationElement]::FromHandle($handle)
$textCondition = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
    [System.Windows.Automation.ControlType]::Text)
$groupCondition = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
    [System.Windows.Automation.ControlType]::Group)

# The criterion says the quota board states its evidence honestly, so the state pill is
# read rather than the empty-board text. Without this the script reported a pass while
# the card showed a green Ready with no reason beside "Quota evidence unavailable" --
# a check that cannot fail on the thing it exists to guard.
# The one combination the criterion forbids. A filled board reporting Ready is a pass, and
# so is an empty board that says why -- what must never appear is a finished answer to a
# question that was never successfully asked. Kept as a pure decision over two strings so
# it can be exercised with both outcomes without a build.
function Test-QuotaDishonesty {
    param([string]$State, [string]$Board)
    return ($State -match '^Ready($|:)') -and ($Board -like '*evidence unavailable*')
}

function Get-PlanUsageState {
    $card = $null
    foreach ($e in $automation.FindAll([System.Windows.Automation.TreeScope]::Descendants, $groupCondition)) {
        if ($e.Current.Name -eq 'Plan Usage') { $card = $e; break }
    }
    if (-not $card) { return '(Plan Usage card not found)' }
    foreach ($e in $card.FindAll([System.Windows.Automation.TreeScope]::Descendants, $textCondition)) {
        if ($e.Current.Name -match '^(Ready|Degraded|Waiting|Unavailable)($|:)') {
            return $e.Current.Name
        }
    }
    return '(no state pill)'
}
$header = ""
foreach ($e in $automation.FindAll([System.Windows.Automation.TreeScope]::Descendants, $textCondition)) {
    if ($e.Current.Name -like "Local usage intelligence*") { $header = $e.Current.Name; break }
}
"header: $header"
$today = @()
foreach ($e in $automation.FindAll([System.Windows.Automation.TreeScope]::Descendants, $textCondition)) {
    if ($e.Current.Name -match '^(Cost|Tokens|Events) ') { $today += $e.Current.Name }
}
"today: {0}" -f ($today -join "  |  ")
"plan usage: {0}" -f (Get-PlanUsageState)

# a stranger waits while the first import runs; watch what the numbers become
foreach ($wait in 60, 120, 180) {
    Start-Sleep -Seconds 60
    $header = ""
    foreach ($e in $automation.FindAll([System.Windows.Automation.TreeScope]::Descendants, $textCondition)) {
        if ($e.Current.Name -like "Local usage intelligence*") { $header = $e.Current.Name; break }
    }
    $cells = @()
    foreach ($e in $automation.FindAll([System.Windows.Automation.TreeScope]::Descendants, $textCondition)) {
        if ($e.Current.Name -match '^(Cost|Tokens|Events) ' -or $e.Current.Name -like "*Quota*" -or $e.Current.Name -like "*quota*") {
            $cells += $e.Current.Name
        }
    }
    $planState = Get-PlanUsageState
    "after {0}s :: {1}" -f $wait, $header
    "            {0}" -f ($cells -join "  |  ")
    "            plan usage: {0}" -f $planState
    if (Test-QuotaDishonesty -State $planState -Board ($cells -join ' ')) {
        Get-Process -Name TokenMaster -ErrorAction SilentlyContinue | Stop-Process -Force
        throw "quota board reports $planState while stating it has no evidence"
    }
}

Get-Process -Name TokenMaster -ErrorAction SilentlyContinue | Stop-Process -Force
Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
"cleaned up"
