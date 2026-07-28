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
| P4 | `verify-secret-scan.ps1` is invoked by nothing. CI runs a Pester suite whose case is named "scans clean committed source history and the validated closed package", but that case reads the script as text and regex-matches its flags. Gitleaks has never run against this repository from CI. The source half was run by hand on 2026-07-28 — 0 findings, with three positive controls proving the detector live — so wiring it in should not turn the lane red. It needs a built package, so its home is the release workflow after `package-product.ps1`. | `scripts/verify-secret-scan.ps1`, `scripts/tests/secret-scan.Tests.ps1`, `.github/workflows/tokenmaster-release-artifact.yml` |
| P4 | Around twenty test sites wait on a background runtime by spinning on `thread::yield_now` under a wall-clock deadline, several as short as two seconds. On a two-core runner already running cases in parallel the spin starves the thread it waits for, so the deadline measures the runner rather than the product. Two sites in `runtime_status_contract.rs` were converted to a sleeping poll after they timed out in CI while passing locally; the rest are untouched. **If a second wall-clock deadline flakes, convert the class instead of the instance** — one more single fix is the loop. `backup_ui_latency_contract` is excluded: there the deadline is the assertion. | `fn await_completion` for the shape; `grep -rn yield_now crates/*/tests` for the sites |
| P3 | Charts distinguishing unavailable from a legitimate zero is not test-guarded. The text cells are covered; the visual difference is not, and the row struct is not exported. | `history-view.slint`, `for row in root.rows` |
| P6 | `hide()` → `show()` skips the partial-render cache invalidation that minimize → restore gets, because winit emits no `Occluded` on Windows. Suspected cause of a stale window after restoring from the tray. Reproduce before fixing. | backend behaviour; restore path in `crates/app/src/application.rs` |
| post-tag | `high-contrast` and `reduced-motion` are declared and consumed by the tree but never set from Rust, so both affordances are inert. Confirmed: zero `set_high_contrast` / `set_reduced_motion` call sites. | `main.slint`, `in property <bool> high-contrast` and `reduced-motion` |
| post-tag | Compact-window sizing samples `window().size()` before the window is shown and compares physical pixels against logical constants. Wrong on any non-100% display. | `fn update_compact_window_mode` |
| post-tag | The Dashboard indexes its six sections positionally with nothing enforcing the order. | `dashboard-view.slint`, `out property <DashboardSectionRow> active-section` |
| post-tag | The Win32 tray carries 39 `unsafe` blocks and one SAFETY comment, and lives in the UI crate rather than `platform`, which exists to own OS FFI and documents 23 of its 25 blocks. Moving it there and then adding `#![forbid(unsafe_code)]` to `desktop` is what stops it drifting back. | `crates/desktop/src/native_tray.rs` |
| post-tag | `RegisterClassW`'s result is discarded and `CLASS_REGISTERED` latches true regardless, so a genuine registration failure is permanent and indistinguishable from the benign already-registered case. | `fn register_window_class` |
| post-tag | `show_menu` holds a `&'static Inner` minted from `GWLP_USERDATA` across `TrackPopupMenu`'s nested modal message loop and dereferences it afterwards. Safe today only because nothing dispatched inside that loop can drop the owner. Re-read the pointer after the loop instead. | `fn show_menu`, `fn inner_from_window` |

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
