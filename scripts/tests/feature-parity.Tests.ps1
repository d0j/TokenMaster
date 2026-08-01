Describe "TokenMaster feature-parity ledger" {
    BeforeAll {
        $RepositoryRoot = (Resolve-Path (Join-Path (Split-Path -Parent $PSScriptRoot) "..")).Path
        $LedgerPath = Join-Path $RepositoryRoot "docs\FEATURE_PARITY.md"
        $TraceabilityPath = Join-Path $RepositoryRoot "spec\TRACEABILITY.md"
        $Ledger = Get-Content -LiteralPath $LedgerPath -Raw

        # Six-field rows only, minus the header and the separator. Parsed once so a change
        # that breaks the table shape fails here rather than silently reducing every check
        # below to zero rows.
        $script:Rows = @(
            $Ledger -split "`r?`n" |
                Where-Object { $_ -match '^\|' } |
                ForEach-Object { , ($_ -split '\|' | Select-Object -Skip 1 -First 6) } |
                Where-Object { $_[0].Trim() -ne 'Ref' -and $_[0].Trim() -notmatch '^-+$' }
        )
    }

    # The ledger states its own acceptance rule -- "a validator must prove that every row has
    # a valid owner, delivery gate, and allowed terminal status" -- and nothing in the
    # repository read this file at all. A rule with no reader is a claim about the present
    # that cannot be checked, which is the same defect as a stale document.
    It "parses every ledger row into six fields" {
        $script:Rows.Count | Should -BeGreaterThan 40
        foreach ($row in $script:Rows) {
            $row.Count | Should -Be 6
        }
    }

    It "gives every row a non-empty capability, decision, owner and delivery gate" {
        $offenders = @()
        foreach ($row in $script:Rows) {
            foreach ($index in 0, 1, 2, 3, 4) {
                if ([string]::IsNullOrWhiteSpace($row[$index])) {
                    $offenders += "$($row[0].Trim()): field $index empty"
                }
            }
        }
        $offenders -join '; ' | Should -BeExactly ''
    }

    It "carries only the four statuses the ledger declares" {
        $allowed = @('implemented', 'partial', 'planned', 'rejected')
        $offenders = @()
        foreach ($row in $script:Rows) {
            $status = $row[5].Trim()
            if ($allowed -notcontains $status) {
                $offenders += "$($row[0].Trim()): $status"
            }
        }
        $offenders -join '; ' | Should -BeExactly ''
    }

    # The ledger's owner cells and the requirement authority are two documents, and prose in
    # two places drifts. `spec/TRACEABILITY.md` is the sole requirement-status authority, so
    # an owner naming a requirement that is not there points the reader at nothing.
    It "names only requirements the traceability authority holds" {
        $known = @(
            [regex]::Matches(
                (Get-Content -LiteralPath $TraceabilityPath -Raw), 'TM-[A-Z]+-\d+'
            ) | ForEach-Object { $_.Value } | Sort-Object -Unique
        )
        $known.Count | Should -BeGreaterThan 20
        $offenders = @()
        foreach ($row in $script:Rows) {
            $owners = @([regex]::Matches($row[3], 'TM-[A-Z]+-\d+') | ForEach-Object { $_.Value })
            if ($owners.Count -eq 0) {
                $offenders += "$($row[0].Trim()): owner names no requirement"
                continue
            }
            foreach ($owner in $owners) {
                if ($known -notcontains $owner) {
                    $offenders += "$($row[0].Trim()): $owner is in no traceability row"
                }
            }
        }
        $offenders -join '; ' | Should -BeExactly ''
    }

    # The authority itself. `CLAUDE.md` names `spec/TRACEABILITY.md` the sole
    # requirement-status authority, and an authority with a malformed row or an invented
    # status is read as fact by everything downstream, this ledger included.
    It "keeps the requirement authority four-field with only its declared statuses" {
        $authority = Get-Content -LiteralPath $TraceabilityPath -Raw
        $rows = @(
            $authority -split "`r?`n" |
                Where-Object { $_ -match '^\|' } |
                ForEach-Object { , ($_ -split '\|' | Select-Object -Skip 1 -First 4) } |
                Where-Object {
                    $_[0].Trim() -ne 'Requirement' -and $_[0].Trim() -notmatch '^-+$'
                }
        )
        $rows.Count | Should -BeGreaterThan 30
        $allowed = @('implemented', 'partial', 'planned', 'open evidence')
        $offenders = @()
        foreach ($row in $rows) {
            if ($row.Count -ne 4) {
                $offenders += "$($row[0].Trim()): $($row.Count) fields"
                continue
            }
            if ($row[0].Trim() -notmatch '^`?TM-[A-Z]+-\d+`?$') {
                $offenders += "$($row[0].Trim()): not a requirement id"
            }
            if ($allowed -notcontains $row[1].Trim()) {
                $offenders += "$($row[0].Trim()): status $($row[1].Trim())"
            }
            foreach ($index in 2, 3) {
                if ([string]::IsNullOrWhiteSpace($row[$index])) {
                    $offenders += "$($row[0].Trim()): field $index empty"
                }
            }
        }
        $offenders -join '; ' | Should -BeExactly ''
    }
}
