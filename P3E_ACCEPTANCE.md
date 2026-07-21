# TokenMaster P3-E interactive acceptance

**Status: contract and strict receipt preflight implemented; authenticated packaged-
production evidence and its P6 provenance binding are absent. P3-E developer
implementation is complete, but P3-E interactive acceptance, P6, M0, packaging,
signing, soak, and release are not accepted.**

## Boundary

This gate applies after P6 has produced the exact packaged executable. The current
script preflights an operator-attested receipt; it does not independently prove that an
interactive action occurred or that the supplied binary came from P6. It does not
package, launch, automate, or mutate TokenMaster, the registry,
Explorer, power state, or the operator's session. The interactive run must use a
disposable Windows user profile or disposable VM and must restore its exact pre-state.

The preflight is intentionally available before P6 so the evidence format is fixed and
testable without creating circular authority. Its pass is necessary but never sufficient
for acceptance. P6 must add authenticated producer/package-manifest provenance before a
receipt can close this gate. A pre-package binary, the isolated `tokenmaster-m0` probe,
or a dirty tree remains forbidden.

## Required receipt

The external operator attests `reports/interactive-p3e.json` with schema
`tokenmaster.p3e.interactive.v1`. The receipt is bound to:

- the exact clean Git commit;
- `dirty: false`;
- `executableKind: packaged-production`;
- the SHA-256 of the exact tested executable;
- `disposableHost: true`;
- verified registry pre-state restoration and stopped task-owned processes.

Every required scenario must occur exactly once and report `pass`:

1. tray Show/Hide/Quit;
2. Explorer restart recovery;
3. secondary-instance activation;
4. registered global hotkey;
5. occupied-hotkey conflict degradation;
6. startup enable, readback, real sign-in launch, and disable;
7. startup relocation, explicit repair, and removal;
8. startup access-denied degradation;
9. lock/unlock;
10. sleep/resume;
11. rapid Show/Hide/Dashboard/Compact changes.

Resource evidence starts after at least eight warm-up cycles and covers at least 64
measured cycles. Private-byte growth must be at most 8 MiB. Handles, threads, USER
objects, and GDI objects must not finish above the post-warm-up baseline. The receipt
contains only bounded stable results and counters: no path, command, command output,
raw OS error, registry data, identity, prompt, response, reasoning, or source content.

## Validation

```powershell
pwsh -NoProfile -File scripts\validate-p3e-interactive.ps1 `
  -RepositoryRoot (Get-Location).Path `
  -ReceiptPath reports\interactive-p3e.json `
  -ExecutablePath <exact-packaged-tokenmaster.exe>
```

The preflight is read-only and fail-closed for receipt shape and local identity checks.
It rejects oversized or ambiguous JSON, schema drift, unknown fields, missing or
duplicate scenarios, unsafe/unclean claims, failed rollback claims, insufficient cycles,
resource overage claims, Git identity mismatch, wrong executable name, and SHA-256
mismatch. It emits only `preflight-pass`; it cannot authenticate package provenance or
the truth of external actions. A future P6 producer/manifest validator plus reviewed
external evidence is required to close P3-E. P4 visual/accessibility/DPI/paint evidence,
M0, soak, signing, and release remain independent.
