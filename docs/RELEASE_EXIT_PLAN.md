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
| P3, **needs a decision** | The Today card reports `Cost $0.00` beside `Tokens —` for the same empty day. Traced to `AggregateTokenValue::from_store`: it asks `known_count == 0` before `known_count == event_count`, so a day with no events at all answers `Unavailable` even though `0 == 0` would already have answered a known zero. Cost, for the identical input, answers `CostAvailability::Zero`. **The asymmetry is not an oversight -- it is written down as intended** in `ready_empty_models_is_explicit_without_fabricating_token_evidence`, which asserts `Unavailable` for tokens and `LegitimateZero` for cost against an archive with every usage row cleared. Reordering the branches is a two-line change and makes that contract fail, which is why it is not made here: the two values describe one day and cannot honestly disagree, but choosing which side is right changes what the user sees from `—` to `0`. Recommendation: an observed day with no events is a legitimate zero for both, since nothing is missing when nothing happened, and the reference mockup shows numbers rather than dashes. | `fn from_store` for `AggregateTokenValue` in `crates/query/src/analytics.rs`; the contract in `crates/desktop/tests/models_projection_contract.rs` |
| post-tag | Test helpers wait on a background runtime by spinning on `thread::yield_now` rather than sleeping between polls, which starves the thread being waited on when the machine has fewer cores than the test binary has parallel cases. With every hang bound now at thirty seconds this no longer decides pass or fail, so it is an efficiency item and not a correctness one. `backup_ui_latency_contract` keeps its spin deliberately: it measures elapsed time, and sleep granularity would enter the measurement. | `grep -rn yield_now crates/*/tests` |
| **P3 blocker, half closed** | The window renders nothing where the buffer was cleared: softbuffer reports a stale buffer age on Win32 and Slint's compensating `occluded()` hook is unreachable there, because winit emits no `Occluded` on Windows. Mechanism, measurements and the upstream comment are in `docs/SLINT_NOTES.md`. `fn force_full_repaint` closes the restore path — 27.7 mean luma after, against 48.4 for a move. **A window the user drags is still affected and cannot be fixed from here**: Slint surfaces no move event. Report upstream; until then the drag case stands. | `fn force_full_repaint`; upstream `i-slint-backend-winit/renderer/sw.rs` |
| P3 | One retained scan, id 1762, ran 2894 seconds and ended `failed` having seen 572 of 4016 sources, between neighbours that each completed in about 4 seconds. Whatever stalled it is not visible in the row itself. Recorded because the evidence is in a 32-row history window and will age out. | `usage_scan` retains `SCAN_HISTORY_PER_SCOPE` rows |
| note, **closed** | **Three dashboard layout defects recorded here did not exist.** A clipped Today row, cards cut at the right edge, and a data-less card claiming full height were all produced by the measuring process, not the product: PowerShell was not DPI aware, so `GetWindowRect` returned virtualised coordinates -- 1061x796 for a window that is really 2266x1511 -- and `PrintWindow` drew the full window into a bitmap half its width, cutting the right edge. `MoveWindow` was halved the same way, which pushed the board under its 944px narrow threshold and stacked it into one column. Captured from a process that calls `SetProcessDpiAwarenessContext(-4)` first, the dashboard shows `COST`, `TOKENS` and `EVENTS` together, a two-column board, and no clipping. **Any capture tool used here must declare DPI awareness before it measures anything**, and the sidebar is the cheap check: it declares `168px` and must measure 168 logical pixels. | `scratchpad/probe.ps1` had this right; later scripts dropped it |
| P3 | A white band renders below the content at the bottom of the window, outside every card, seen on a capture of the release build at 2122x1591. It is not the bottom tab bar, which draws above it. Either an unpainted region of the surface or a layout leaving slack under the last row; distinguish by resizing and watching whether the band scales. | capture `views.png`; start at the root `VerticalLayout` in `main.slint` |
| post-tag | `high-contrast` and `reduced-motion` are declared and consumed by the tree but never set from Rust, so both affordances are inert. Confirmed: zero `set_high_contrast` / `set_reduced_motion` call sites. | `main.slint`, `in property <bool> high-contrast` and `reduced-motion` |
| post-tag | The Dashboard indexes its six sections positionally with nothing enforcing the order. | `dashboard-view.slint`, `out property <DashboardSectionRow> active-section` |
| post-tag | The Win32 tray carries 39 `unsafe` blocks and one SAFETY comment, and lives in the UI crate rather than `platform`, which exists to own OS FFI and documents 23 of its 25 blocks. Moving it there and then adding `#![forbid(unsafe_code)]` to `desktop` is what stops it drifting back. | `crates/desktop/src/native_tray.rs` |

## Direction, and where the idle cost is not

The trend card's area fill and its two-dot legend are **confirmed on a live window**: the
teal and blue series each carry a soft fill, and `tokens` and `cost` are named beside the
`Ready` pill. Both had been committed on their unit tests alone.

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

**The idle cost is closed, and it was never rendering.** Two defects held a core, and both
were found by measuring writes rather than by reading code -- three hypotheses taken from the
source were wrong first, one of them twice.

The first was a transaction boundary. Presence is recorded for every source a scan sees,
changed or not, and each source carried its own transaction: 8032 of 9564 journal frames landed
on `usage_source` across 237 distinct pages, one commit per file. Batching them cost 16.0 MB per
pass down to 1.31 MB.

The second was the shape of a hint. The watched root is the profile root, not the sessions
directory, and the tools that own that root write into it constantly -- of eight files changed
under a real `~/.codex` in ten minutes, seven were outside both source directories. Any such path
failed the whole targeted batch, which became a full walk of all 4016 sources. Skipping paths
that can never be usage data -- while still reconciling a path whose file merely disappeared --
removed the walk.

Measured end to end on the live 816 MB archive, same script both times: journal over 120 seconds
fell from 165 MB to 4.91 MB, processor time from 151.8-171.1 seconds per 180 to 27.4 per 120, and
the process now reaches zero processor use about forty seconds after launch instead of holding a
core indefinitely.

Threads already carry names; the standard library sets them. A probe that reports otherwise is
asking `GetThreadDescription` for the wrong access right.

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
