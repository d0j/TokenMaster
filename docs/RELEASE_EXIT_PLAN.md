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
| **P3 blocker** | **Ingestion rewrites the same pages forever.** Sampled every five seconds for two minutes on the release build with an 816 MB archive: the database grew 30 KB while the write-ahead log went from 16 MB to 181 MB — about 16 MB every ten seconds, monotonic — and the process held 80–190% of a core throughout. Work is being redone, not done. Threads confirm where: summed over every thread of that name, `tokenmaster-refresh` takes 79.1% of a core over 180 seconds, `main` 8.1%, and the watcher, scheduler and reminder threads exactly nil. Three consecutive launches cost 151.8, 165.8 and 171.1 processor seconds, so nothing amortises. The worker's outer loop is a blocking `recv`, so either one refresh never finishes or submissions arrive back to back; the flat database against a growing log says the writes are re-applications of rows that already exist, which points at the idempotence of re-ingestion rather than at scheduling. The log grows in steps, not smoothly -- 16.14, 16.68, 32.81, 33.04, 49.07, 49.09 -- so it is a cycle of roughly ten seconds writing about 16 MB, on the order of four thousand pages a pass. Neither the scheduler (60 s degraded, 900 s healthy) nor the backup policy (six hours minimum) has that period, so the driver is something else. The unchanged-source short circuit in `crates/runtime/src/incremental.rs` looks complete but requires `BatchState::SnapshotEnd`, which is the first thing to check. The unbounded log is its own hazard: six gigabytes an hour. **Rewritten three times because each earlier probe was narrower than the thing it measured** — the reliable signal is whole-process CPU plus a sum over *all* threads sharing a name, since two live threads are called `tokenmaster-refresh`. | `fn run_worker`, `crates/engine/src/worker.rs`; the write path in `crates/store/src/usage/write.rs` |
| post-tag | `high-contrast` and `reduced-motion` are declared and consumed by the tree but never set from Rust, so both affordances are inert. Confirmed: zero `set_high_contrast` / `set_reduced_motion` call sites. | `main.slint`, `in property <bool> high-contrast` and `reduced-motion` |
| post-tag | The Dashboard indexes its six sections positionally with nothing enforcing the order. | `dashboard-view.slint`, `out property <DashboardSectionRow> active-section` |
| post-tag | The Win32 tray carries 39 `unsafe` blocks and one SAFETY comment, and lives in the UI crate rather than `platform`, which exists to own OS FFI and documents 23 of its 25 blocks. Moving it there and then adding `#![forbid(unsafe_code)]` to `desktop` is what stops it drifting back. | `crates/desktop/src/native_tray.rs` |

## Direction, and where the idle cost is not

`docs/UI_REFERENCE.md` holds the owner's Dashboard mockup transcribed as a binding
specification. It is the standard: where the built product differs from it, the product is
wrong. Its second list -- the elements blocked on data the projection does not carry -- is
the more valuable half, because each entry is a fact the product already computes and
discards.

The UI stack is settled and was checked rather than assumed. Six research angles across
forums, issue trackers and shipped-product retrospectives, with the strongest claims put
through refutation. Verdict: keep Slint with the Skia renderer. Every alternative loses on
one hard requirement -- unavailable, partial and legitimate-zero must stay distinguishable
in the accessible label. iced has carried an open accessibility issue since 2020, GPUI is
invisible to JAWS and NVDA, Tauri and Dioxus are a WebView, Xilem is alpha. egui is the only
real fallback and it has no hot reload at all.

**Turn on `slint/live-preview`.** Measured on this repository: generated Rust falls from
15,997,690 bytes to 296,507 -- fifty-four times smaller -- with an identical API surface,
and a `tokens.slint` edit stops costing a thirteen-second rebuild. It is a non-default
feature and must never reach a release build.

**The cold-start cost is answered.** It was never idling and never Slint: an empty Slint
window with this repository's exact features costs 0% of a core, and thread-name profiling
puts the work in `tokenmaster-refresh` -- about 184% of a core for the first 43 seconds,
then 197 consecutive samples at zero. Renderer, animation and `accessibility` were each
ruled out by measurement before that. The remaining question is narrow: whether every
launch redoes the scan or only the first.

Threads already carry names; the standard library sets them. A probe that reports otherwise
is asking `GetThreadDescription` for the wrong access right.

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
