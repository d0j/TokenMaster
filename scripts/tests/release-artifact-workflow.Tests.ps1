Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

Describe 'release artifact attestation workflow' {
    BeforeAll {
        $RepositoryRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
        $WorkflowPath = Join-Path $RepositoryRoot '.github\workflows\tokenmaster-release-artifact.yml'
    }

    It 'limits the unsigned package attestation to trusted release contexts' {
        Test-Path -LiteralPath $WorkflowPath -PathType Leaf | Should -BeTrue
        $workflow = Get-Content -LiteralPath $WorkflowPath -Raw

        $workflow | Should -Match '(?m)^\s*workflow_dispatch:\s*$'
        $workflow | Should -Match '(?m)^\s*push:\s*$'
        $workflow | Should -Match '(?m)^\s*tags:\s*$'
        $workflow | Should -Match '(?m)^\s*-\s*"v\*"\s*$'
        $workflow | Should -Not -Match '(?m)^\s*pull_request:'
        $workflow | Should -Match 'github\.event_name == ''push'''
        $workflow | Should -Match "github\.ref == format\('refs/heads/\{0\}', github\.event\.repository\.default_branch\)"
    }

    It 'uses minimal permissions and immutable actions for the canonical ZIP provenance' {
        $workflow = Get-Content -LiteralPath $WorkflowPath -Raw

        $workflow | Should -Match '(?m)^permissions:\r?\n\s+contents:\s+read\s*$'
        $workflow | Should -Match '(?m)^\s+id-token:\s+write\s*$'
        $workflow | Should -Match '(?m)^\s+attestations:\s+write\s*$'
        $workflow | Should -Not -Match '(?m)^\s+contents:\s+write\s*$'
        $workflow | Should -Match 'actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7'
        $workflow | Should -Match 'actions/attest@f7c74d28b9d84cb8768d0b8ca14a4bac6ef463e6 # v4\.2\.0'
        $workflow | Should -Match 'actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a # v7'
        $workflow | Should -Match '(?m)^\s+push-to-registry:\s+false\s*$'
        $workflow | Should -Match '(?m)^\s+create-storage-record:\s+false\s*$'
        $workflow | Should -Not -Match '(?m)^\s+artifact-metadata:\s+write\s*$'
    }

    It 'packages then attests and uploads only the canonical unsigned ZIP and receipt' {
        $workflow = Get-Content -LiteralPath $WorkflowPath -Raw
        $packageIndex = $workflow.IndexOf('./scripts/package-product.ps1', [StringComparison]::Ordinal)
        $attestIndex = $workflow.IndexOf('actions/attest@', [StringComparison]::Ordinal)
        $uploadIndex = $workflow.IndexOf('actions/upload-artifact@', [StringComparison]::Ordinal)

        $packageIndex | Should -BeGreaterThan -1
        $attestIndex | Should -BeGreaterThan $packageIndex
        $uploadIndex | Should -BeGreaterThan $attestIndex
        $workflow | Should -Match '(?m)^\s+subject-path:\s+dist/TokenMaster-\*-windows-x64-unsigned\.zip\s*$'
        $workflow | Should -Match '(?m)^\s+dist/TokenMaster-\*-windows-x64-unsigned\.zip\s*$'
        $workflow | Should -Match '(?m)^\s+dist/TokenMaster-\*-windows-x64-unsigned\.receipt\.json\s*$'
        $workflow | Should -Not -Match '(?m)^\s+dist/TokenMaster-\*-windows-x64-signed\.zip\s*$'
    }
}
