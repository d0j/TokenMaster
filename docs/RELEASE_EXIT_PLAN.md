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

1. Commit the green import/runtime slice as one intentional clean release commit.
2. Build and verify the deterministic unsigned portable package from that exact commit.
3. Run the pinned committed-history and closed-package secret scans.
4. Obtain one physical Windows UI confirmation on the packaged executable.

Do not start P4, provider-host, audit-parser, signing, M0, or remote receipt work before
the preceding step is closed.

## Current evidence and open blockers

- Schema v14, exact replay-parent index selection, and fixed-capacity statement caching
  reduce one measured 256-observation transaction from 74,592 ms to 291 ms.
- The synthetic 49,152-event parser reaches 4,112 events/s and the 4,096-event runtime
  cold contract completes in 770 ms.
- A consistent copy of the real approximately 5.0 GiB history advanced 8,121 events
  during a 120-second bounded run and shut down cleanly in 122.30 seconds. This proves
  progress and cancellation, not complete cold import.
- Clean-root, format, warnings-as-errors workspace Clippy, the deterministic complete
  locked workspace test/doc-test gate, and the final MSVC release build pass.
- Fresh portable import is responsive and publishes current-day usage within the bounded
  local smoke window. Newest-first traversal is bounded per directory and does not change
  accounting authority.
- The startup Dashboard exposes a truthful in-app import state while retaining available
  safe data.
- A software-rendered pixel contract proves the startup Dashboard frame is non-black and
  non-uniform. Native `PrintWindow` of the exact packaged executable independently
  proves a real non-black, non-uniform physical shell/Dashboard frame.
- The integrated restart-safe replay and current-day first-use slice passed one
  independent review, its focused correction contracts, and the complete workspace
  quality gate.
- The clean-commit deterministic unsigned package, package validator, committed-history
  secret scan, closed-ZIP secret scan, and isolated packaged functional launch pass.
- Windows Automation exposes real current metrics, routes, import state, and graph
  semantics. Native capture retains an older compositor frame, so it is not represented
  as a synchronized image of those later values; local physical paint itself is green.
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
| UI frame | software pixel buffer plus native packaged-window capture | green |
| Core UI functions | compiled route/control contracts and packaged functional check | green |
| Integrated correctness | focused replay/runtime/store contracts | green |
| Workspace quality | clean-root, fmt, strict Clippy, full locked tests | green, 21m03s |
| Git | intentional clean commit | green |
| Package | deterministic unsigned portable ZIP plus exact secret receipt | green |
| Stable release | signing plus external acceptance and operator-started soak | deferred/external |
