# TokenMaster release exit plan

This file is the per-cycle delivery rail until the first accepted release. Read it after
`docs/HANDOFF.md`. Repository contracts and acceptance documents remain authoritative.

## Current milestone

Produce one locally accepted, deterministic, unsigned Windows x64 portable `0.1.0`
release candidate under Apache-2.0. Do not call it a stable or signed release.

The explicitly deferred 24-hour soak is not part of this milestone. Signing,
authenticated clean-room/P3-E evidence, and physical DPI/accessibility checks remain
separate external acceptance boundaries.

## Shortest critical path

1. Keep the current safe publication visible and make active recovery/import explicit.
2. Prove a fresh portable launch reaches current local usage quickly with bounded memory.
3. Close the existing restart-safe replay WIP and run focused contracts.
4. Run the full workspace gate once.
5. Commit one intentional clean release slice.
6. Build and verify the deterministic unsigned portable package from that exact commit.
7. Obtain one physical Windows UI confirmation on the packaged executable.

Do not start P4, provider-host, audit-parser, signing, M0, or remote receipt work before
the preceding step is closed.

## Current evidence and open blockers

- Fresh portable import is responsive and publishes current-day usage within the bounded
  local smoke window. Newest-first traversal is bounded per directory and does not change
  accounting authority.
- The startup Dashboard exposes a truthful in-app import state while retaining available
  safe data.
- A software-rendered pixel contract proves the startup Dashboard frame is non-black and
  non-uniform. The available Windows window-capture API still returns black, so physical
  paint remains unconfirmed rather than diagnosed as a renderer defect.
- The integrated restart-safe replay and current-day first-use slice passed one
  independent review, its focused correction contracts, and the complete workspace
  quality gate.
- The clean-commit deterministic unsigned package, package validator, committed-history
  secret scan, closed-ZIP secret scan, and isolated packaged functional launch pass.
- Windows UI Automation exposes real current metrics, routes, import state, and graph
  semantics. Trustworthy physical pixels remain open because the available capture API
  fails independently of the proven software pixel buffer and accessibility tree.
- Stable-release acceptance additionally requires signing and external acceptance. The
  24-hour soak stays deferred until the operator explicitly starts it.

## Anti-loop controls

At the start and end of every autonomous cycle, record:

1. the user-visible milestone;
2. the single shortest release-critical outcome;
3. the real remaining blockers;
4. whether the cycle changed product behavior, correctness, required evidence, or only
   audit machinery.

Stop and reconsider the path when any condition is true:

- 60 minutes pass without closing or materially reducing a product/release blocker;
- the same failing command or hypothesis is attempted twice without changed evidence;
- two consecutive rounds change only tests, audits, or docs;
- a reviewer proposes work without a demonstrated production, security, data-loss, or
  mandatory receipt defect;
- a full workspace gate is about to be repeated before the product slice changed.

`AUDIT_HARDENING_LOOP` handling in `AGENTS.md` is mandatory. A black capture is not a
renderer defect unless physical presentation or a real pixel buffer reproduces it.

## Per-cycle acceptance ledger

| Gate | Required evidence | State |
|---|---|---|
| Import truth | visible active state plus retained safe publication | focused green |
| Time to useful data | fresh portable current-day event publication | local green |
| Memory | bounded, stable smoke samples | local green |
| UI frame | non-black/non-uniform software pixel buffer | focused green |
| Core UI functions | compiled route/control contracts and packaged functional check | green; physical pixels open |
| Integrated correctness | focused replay/runtime/store contracts | green |
| Workspace quality | clean-root, fmt, strict Clippy, full locked tests | green, 21m03s |
| Git | intentional clean commit | green |
| Package | deterministic unsigned portable ZIP plus exact secret receipt | green |
| Stable release | signing plus external acceptance and operator-started soak | deferred/external |
