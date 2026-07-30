# TokenMaster P3-E interactive acceptance

**Status: contract, strict receipt preflight, and local P6 package-provenance binding
are implemented. Authenticated external interactive evidence is absent. P3-E developer
implementation is complete, but P3-E interactive acceptance, signing, M0, soak, RC,
and release are not accepted.**

## Boundary

This gate applies after P6 has produced the exact packaged executable. The current
script preflights an operator-attested receipt; it does not independently prove that an
interactive action occurred or that the supplied binary came from P6. It does not
package, launch, automate, or mutate TokenMaster, the registry,
Explorer, power state, or the operator's session. The interactive run must use a
disposable Windows user profile or disposable VM and must restore its exact pre-state.

The preflight requires the deterministic P6 ZIP and its producer receipt. It validates
the closed package stage and binds package hash, build identity, clean Git commit,
packaged executable hash, exact tested executable, and interactive receipt. Its pass is
necessary but never sufficient for acceptance: it does not authenticate the external
operator or prove the interactive actions. A pre-package binary, the isolated
`tokenmaster-m0` probe, a forged operator claim, or a dirty tree remains forbidden.

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
  -ExecutablePath <exact-tested-packaged-tokenmaster.exe> `
  -PackagePath dist\TokenMaster-0.1.0-windows-x64-unsigned.zip `
  -PackageReceiptPath dist\TokenMaster-0.1.0-windows-x64-unsigned.receipt.json
```

The preflight is read-only and fail-closed for receipt shape, local identity, and
package provenance. It rejects oversized or ambiguous JSON/ZIP input, schema drift,
unknown fields, missing or duplicate scenarios, unsafe/unclean claims, failed rollback
claims, insufficient cycles, resource overages, Git/build/package/executable identity
drift, unsafe package entries, and checksum/manifest mismatch. It emits only
`preflight-pass`; it cannot authenticate the operator or truth of external actions.
Reviewed external evidence is still required to close P3-E. P4 visual/accessibility/
DPI/paint evidence, M0, soak, signing, and release remain independent.
