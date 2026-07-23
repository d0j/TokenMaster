param(
    [string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

try {
    $root = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $RepositoryRoot).Path)
    $workflowRoot = Join-Path $root '.github\workflows'
    if (-not (Test-Path -LiteralPath $workflowRoot -PathType Container)) {
        throw 'TM-ACTIONS-WORKFLOWS'
    }
    $workflows = @(Get-ChildItem -LiteralPath $workflowRoot -File |
        Where-Object { $_.Extension -in @('.yml', '.yaml') })
    if ($workflows.Count -eq 0 -or $workflows.Count -gt 64) {
        throw 'TM-ACTIONS-WORKFLOWS'
    }
    $remoteCount = 0
    foreach ($workflow in $workflows) {
        if ($workflow.Length -gt 1048576) {
            throw 'TM-ACTIONS-WORKFLOW'
        }
        foreach ($line in [IO.File]::ReadAllLines($workflow.FullName)) {
            if ($line -notmatch '^\s*(?:-\s*)?uses:\s*(\S+?)(?:\s+#\s*\S.*)?\s*$') {
                if ($line -match '^\s*(?:-\s*)?uses:') {
                    throw 'TM-ACTIONS-REFERENCE'
                }
                continue
            }
            $reference = $Matches[1]
            if ($reference.StartsWith('./', [StringComparison]::Ordinal)) {
                if ($reference.Contains('..') -or $reference.Contains('\')) {
                    throw 'TM-ACTIONS-REFERENCE'
                }
                continue
            }
            if ($reference -notmatch '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_./-]+)?@[0-9a-f]{40}$') {
                throw 'TM-ACTIONS-REFERENCE'
            }
            $remoteCount++
        }
    }
    if ($remoteCount -eq 0) {
        throw 'TM-ACTIONS-REFERENCE'
    }
    Write-Output 'immutable-actions-pass'
    exit 0
}
catch {
    Write-Output 'immutable-actions-fail'
    exit 1
}
