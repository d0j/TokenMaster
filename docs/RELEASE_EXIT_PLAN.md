# TokenMaster release exit plan

The per-cycle delivery rail until the first accepted release. Repository contracts and
acceptance documents remain authoritative.

## Current milestone

One downloadable, deterministic, unsigned Windows x64 portable `0.1.0` under Apache-2.0.
Not a stable release and not signed.

Acceptance is a stranger, not a receipt: a machine that has never built this repository
downloads the packaged executable, launches it, and sees its own Codex history with a
lifetime total and a working quota board.

Before any parity claim, every row in `docs/FEATURE_PARITY.md` must be implemented or
explicitly rejected under its normative rationale.

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

## Recorded, with a phase

Defects found while working on something else. Each is deferred deliberately, not
forgotten; the phase is the commitment. Background and mechanism are in
`docs/SLINT_NOTES.md`.

**Anchors are symbols, never line numbers.** Every line reference this table once carried
had drifted out of date, and two entries described defects that had already been fixed —
a stale rail is worse than no rail, because it is trusted. A function or property name
survives edits and is greppable.

| Phase | Item | Anchor |
|---|---|---|
| P3 | The Today card reports `Cost $0.00` beside `Tokens —` while the evidence chip reads `Fresh · Authoritative` and the event count reads `0 events`. Both cells come from one `header`, and both formatters render a missing value as `—`, so the disagreement is in the domain: for the same empty day cost claims a legitimate zero and tokens claim no evidence. One of them is wrong, and the product's whole claim is that these three states stay distinct. Observed on a running build, not inferred. | `fn format_cost` and `fn format_tokens` are honest; start at `dashboard.header()` |
| post-tag | Test helpers wait on a background runtime by spinning on `thread::yield_now` rather than sleeping between polls, which starves the thread being waited on when the machine has fewer cores than the test binary has parallel cases. With every hang bound now at thirty seconds this no longer decides pass or fail, so it is an efficiency item and not a correctness one. `backup_ui_latency_contract` keeps its spin deliberately: it measures elapsed time, and sleep granularity would enter the measurement. | `grep -rn yield_now crates/*/tests` |
| **P3 blocker, half closed** | The window renders nothing where the buffer was cleared: softbuffer reports a stale buffer age on Win32 and Slint's compensating `occluded()` hook is unreachable there, because winit emits no `Occluded` on Windows. Mechanism, measurements and the upstream comment are in `docs/SLINT_NOTES.md`. `fn force_full_repaint` closes the restore path — 27.7 mean luma after, against 48.4 for a move. **A window the user drags is still affected and cannot be fixed from here**: Slint surfaces no move event. Report upstream; until then the drag case stands. | `fn force_full_repaint`; upstream `i-slint-backend-winit/renderer/sw.rs` |
| **P3 blocker** | **An idle window costs most of a core, in bursts.** Sampled every ten seconds from a cold start of the release build: 103, 107, 108, 105, 145, 42, 0, 72, 9, 64, 86, 85 percent of one core, with private memory rising from 26 to 48 MB over the same two minutes. So it is periodic work, not a spin — consistent with the reconciliation TM-FUNC-008 requires even while the watcher is healthy, but far too expensive to run behind a tray icon. Gating the only `animation-tick()` did not change it. **An earlier version of this entry said one core continuously, for the life of the process. That was three six-second samples that happened to land inside bursts: a short window turns a pulsing signal into a flat one.** | start at the watcher and the periodic reconcile, not at the renderer |
| post-tag | `high-contrast` and `reduced-motion` are declared and consumed by the tree but never set from Rust, so both affordances are inert. Confirmed: zero `set_high_contrast` / `set_reduced_motion` call sites. | `main.slint`, `in property <bool> high-contrast` and `reduced-motion` |
| post-tag | The Dashboard indexes its six sections positionally with nothing enforcing the order. | `dashboard-view.slint`, `out property <DashboardSectionRow> active-section` |
| post-tag | The Win32 tray carries 39 `unsafe` blocks and one SAFETY comment, and lives in the UI crate rather than `platform`, which exists to own OS FFI and documents 23 of its 25 blocks. Moving it there and then adding `#![forbid(unsafe_code)]` to `desktop` is what stops it drifting back. | `crates/desktop/src/native_tray.rs` |

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

`scripts/validate-p3e-interactive.ps1` belongs here: it takes a built executable and a
built package, so it is an operator-run gate against a packaged artifact, not a CI step.
Its acceptance criteria are in `P3E_ACCEPTANCE.md`. Nothing invokes it automatically, and
nothing should.
