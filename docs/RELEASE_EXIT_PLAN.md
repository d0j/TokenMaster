# TokenMaster release exit plan

The per-cycle delivery rail until the first accepted release. Repository contracts and
acceptance documents remain authoritative.

## Current milestone

One downloadable, deterministic, unsigned Windows x64 portable `0.1.0` under Apache-2.0.
Not a stable release and not signed.

Acceptance is a stranger, not a receipt: a machine that has never built this repository
downloads the packaged executable, launches it, and sees its own Codex history with a
lifetime total and a working quota board.

## Sequence

Do not start a phase before the previous one meets its criterion.

| Phase | Outcome | Criterion |
|---|---|---|
| P0 | Unblock the gate | 17 workspace members; no `windows-gnu`; verify job under 45 min |
| P1 | Correctness gate | A re-classified fork keeps its proved replay prefix |
| P2 | Close the schema one-way door | Byte-exact `sqlite_master` fixture holds; `user_version = 14` rejects |
| P3 | Product surface | All 11 routes reachable by mouse; no cost renders with three or more decimals; unavailable and legitimate zero render differently; a lifetime total exists |
| P4 | **Tag `v0.1.0`** | The stranger test above |
| P5 | Agent access | `tm today` equals a direct SQL sum, under 500 ms warm |
| P6 | Ingestion, capped | Cold import under 300 s median with a byte-identical selected-event set |
| P7 | Claude provider | Two `provider_id` values under one writer lease |

## Abort rules

These are binding. They exist because this project produced 543 commits and zero
releases, and every stop condition it had was judged by the agent producing the work.

- **Migration ladder.** If the byte-exact schema diff does not converge within one extra
  day, keep the ladder and tag anyway. Dead migration code is a cost optimisation; it
  never holds the first release hostage.
- **Cache-write columns and the ladder collapse share one commit window.** Collapsing
  first and adding the columns later rebuilds a ladder of one.
- **Cold-import wall time is not a gate.** No normative requirement states one, and
  ADR-093 records that the WhereMyTokens comparison is not a like-for-like workload.
  Keep the measured number as a baseline with a no-regression guard.
- **Two failed attempts at one construction** means pinning the invariant one level
  down, not a third attempt.
- **A defect found outside the current item** is recorded and deferred, never fixed
  inline.

Record an `AUDIT_HARDENING_LOOP` trigger and its disposition here before continuing.

## Deferred and external

Signing, the 24-hour soak, physical DPI/accessibility/screen-reader passes, and
authenticated clean-room evidence are outside this milestone. The soak harness was
deleted with the probe binary it measured; it must be rewritten against the packaged
executable before any soak is run.
