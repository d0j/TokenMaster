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
| P2 | Close the schema one-way door | Byte-exact `sqlite_master` fixture holds; one version past `USAGE_SCHEMA_VERSION` rejects as `SchemaTooNew` |
| P3 | Product surface | **met** — all four checks below |
| P4 | **Tag `v0.1.0`** | The stranger test above |
| P5 | Agent access | `tm today` equals a direct SQL sum, under 500 ms warm |
| P6 | Ingestion, capped | Cold import under 300 s median with a byte-identical selected-event set |
| P7 | Claude provider | Two `provider_id` values under one writer lease |

### P3, checked rather than assumed

Every clause of the criterion now has evidence, and two of them had none before.

- **All 11 routes reachable.** Verified from outside the process against the running
  build: the accessibility tree exposes eleven route buttons, each was activated through
  its own invoke action, and the shell header followed to the named route in ten cases.
  `Compact Widget` replaces the shell, so it was confirmed by the window resizing from
  2122x1591 to 866x1191. Activation used `InvokePattern` rather than a synthetic click --
  those did not land because the window was not foreground, which is a limit of the
  harness and not of the product -- and it runs the same callback a click does, on
  controls that each own a rectangle inside the window.
- **No cost renders with three or more decimals.** Guarded by a sweep over every micro
  value up to a dollar, then powers of ten and their neighbours to `u64::MAX`. It failed
  on its first run: `format_usd_micros` added its rounding term before dividing and
  overflowed near the top of the range, which a release build would have wrapped into an
  enormous cost rendered as a trivial one. Fixed and pinned.
- **Unavailable and legitimate zero render differently.** Both formatters behaved and
  nothing held them to it. Now pinned, and verified by breaking it.
- **A lifetime total exists.** Built across store, query, product, projection and shell;
  the header reads `16,345,419,812 all time` on a live window.

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
| **P3 exit, closed** | A lifetime total now exists and is on screen: the shell header reads `Local usage intelligence · Dashboard · 16,345,419,812 all time`, confirmed on a live window at 2122x1591. It travels the full chain rather than around it -- an unbounded aggregate over `usage_time_rollup` in the store, `LifetimeUsage` in the query layer, its own `ProductSnapshot` section, a projection field, and a shell property, because every number this interface shows arrives by snapshot. | `fn lifetime_totals`; `shell-lifetime-tokens` in `main.slint` |
| **P4 blocker, closed** | The Today card filled with real numbers and then went back to claiming no evidence while ingestion continued. Cause: a refresh publishes its status first and its usage answers second under one attempt number, and invalidation raised the section's attempt to the status's, so `classify` met an equal number and coalesced -- the analytics computed in that very refresh was discarded. During a first import the dataset identity changes every pass, so the card never refilled. The section now keeps its own attempt; an answer against the superseded dataset is still refused by the compatibility predicate, where that check belongs. Verified on a fresh extraction of the shipped package at three marks: `Tokens 197,189,455` and `Events 1,412 events` at 60, 120 and 180 seconds, with `Cost $21.83` appearing, while the lifetime total climbed from 4.46 to 11.64 billion. The same marks read two dashes before the fix. | `fn invalidate_if` in `crates/product/src/reducer.rs` |
| **P4, needs a decision** | **The quota board says its evidence is unavailable, and no location this code could guess would fix it.** Quota is not read from the archive; the runtime spawns `codex.exe app-server --stdio`. Discovery reads `PATH` and nothing else, and on this machine `codex` on `PATH` is only the npm shims `codex.cmd` and `codex.ps1`. Two earlier readings of this were wrong and both were wrong for the same reason -- reasoning from `PATH` and the source instead of looking at the disk. It is **not** true that no `codex.exe` exists: there are four, at three versions. `%LOCALAPPDATA%\OpenAI\Codex\bin` holds `0.130.0-alpha.5`, `~/.codex/.sandbox-bin` and `~/.codex/plugins/.plugin-appserver` hold `0.146.0-alpha.3.1`, and the npm package vendors `0.144.1`, which is what `codex --version` answers and therefore what the user actually runs. A fix that searched the `%LOCALAPPDATA%` directory was written, pinned by contract, and **reverted**: it selects `0.130.0-alpha.5`, on which `codex login status` fails outright with `invalid type: map, expected a string`, so the guess lands on the one binary that cannot work. **Resolving the shim is not available either** -- `codex.cmd` reads `"%_prog%" "%dp0%\node_modules\@openai\codex\bin\codex.js"` with `_prog` set to `node`, so following it means executing a script interpreter, which is exactly what "script/PATHEXT/package-wrapper rejection" forbids. What is left is not a guess: `CodexQuotaRuntimeConfig::with_executable` already accepts an explicit executable and is already honoured over discovery -- **nothing exposes it to the user.** Two ways out: surface that setting, so the answer comes from the person who knows which Codex is theirs; or narrow the criterion to a quota board that states its evidence honestly, which it does. Measured while deciding: the Plan Usage card renders `Ready` with no reason codes and zero rows, so a poll failure is not reaching the section at all -- it lands in runtime health, which no route displays. | `fn resolve_current_command` in `crates/runtime/src/quota/config.rs`; `fn poll` in `crates/runtime/src/provider_quota.rs` |
| P3, **needs a decision** | The Today card reports `Cost $0.00` beside `Tokens —` for the same empty day. Traced to `AggregateTokenValue::from_store`: it asks `known_count == 0` before `known_count == event_count`, so a day with no events at all answers `Unavailable` even though `0 == 0` would already have answered a known zero. Cost, for the identical input, answers `CostAvailability::Zero`. **The asymmetry is not an oversight -- it is written down as intended** in `ready_empty_models_is_explicit_without_fabricating_token_evidence`, which asserts `Unavailable` for tokens and `LegitimateZero` for cost against an archive with every usage row cleared. Reordering the branches is a two-line change and makes that contract fail, which is why it is not made here: the two values describe one day and cannot honestly disagree, but choosing which side is right changes what the user sees from `—` to `0`. Recommendation: an observed day with no events is a legitimate zero for both, since nothing is missing when nothing happened, and the reference mockup shows numbers rather than dashes. | `fn from_store` for `AggregateTokenValue` in `crates/query/src/analytics.rs`; the contract in `crates/desktop/tests/models_projection_contract.rs` |
| post-tag | **The owner revised the Dashboard mockup and it is transcribed as the new standard.** Six elements the product already builds are now wrong against it: the top strip becomes four cells with `CACHE HIT` promoted out of Plan Usage; Plan Usage's sub-cards become `IN / OUT`, `BURN / DAY`, `RESETS LEFT`; `NET` leaves the Code Output metric row for a centred caption under its chart while `$ / 100 LINES` takes the fourth slot; the wordmark turns green; `Go to route` leaves the header; and the bottom bar stops being four navigation entries and becomes a daemon/queue/archive status line. Two whole cards are new -- a Dashboard `Sessions` table of window, tokens, cost and model, and `Cost by Model` with per-model share bars and call/in/out/cache captions. The instruction attached to the mockup is that the layout is the specification and **every value comes from TokenMaster's own archive**, so a figure transcribed from the image into the product would be a fabrication of exactly the kind this interface exists to refuse. | `docs/UI_REFERENCE.md`, its "Superseded by this revision" list |
| post-tag | Test helpers wait on a background runtime by spinning on `thread::yield_now` rather than sleeping between polls, which starves the thread being waited on when the machine has fewer cores than the test binary has parallel cases. With every hang bound now at thirty seconds this no longer decides pass or fail, so it is an efficiency item and not a correctness one. `backup_ui_latency_contract` keeps its spin deliberately: it measures elapsed time, and sleep granularity would enter the measurement. | `grep -rn yield_now crates/*/tests` |
| **P3 blocker, half closed** | The window renders nothing where the buffer was cleared: softbuffer reports a stale buffer age on Win32 and Slint's compensating `occluded()` hook is unreachable there, because winit emits no `Occluded` on Windows. Mechanism, measurements and the upstream comment are in `docs/SLINT_NOTES.md`. `fn force_full_repaint` closes the restore path — 27.7 mean luma after, against 48.4 for a move. **A window the user drags is still affected and cannot be fixed from here**: Slint surfaces no move event. Report upstream; until then the drag case stands. | `fn force_full_repaint`; upstream `i-slint-backend-winit/renderer/sw.rs` |
| post-tag, **evidence expired** | A retained scan, id 1762, once ran 2894 seconds and ended `failed` having seen 572 of 4016 sources, between neighbours that each completed in about 4 seconds. It has since aged out of the 32-row history window, which now holds scans 1826 to 1857, **all 32 of them `complete`**, the last eight lasting 1.0 to 1.9 seconds. So the anomaly cannot be investigated from the archive any longer and has not recurred across at least sixty-five scans. Kept as a note rather than a task: if a scan ever fails again the row is the place to look, and a scan is now expected to take under two seconds. | `usage_scan`, bounded by `SCAN_HISTORY_PER_SCOPE` |
| note, **closed** | **Three dashboard layout defects recorded here did not exist.** A clipped Today row, cards cut at the right edge, and a data-less card claiming full height were all produced by the measuring process, not the product: PowerShell was not DPI aware, so `GetWindowRect` returned virtualised coordinates -- 1061x796 for a window that is really 2266x1511 -- and `PrintWindow` drew the full window into a bitmap half its width, cutting the right edge. `MoveWindow` was halved the same way, which pushed the board under its 944px narrow threshold and stacked it into one column. Captured from a process that calls `SetProcessDpiAwarenessContext(-4)` first, the dashboard shows `COST`, `TOKENS` and `EVENTS` together, a two-column board, and no clipping. **Any capture tool used here must declare DPI awareness before it measures anything**, and the sidebar is the cheap check: it declares `168px` and must measure 168 logical pixels. | `scripts/stranger-acceptance.ps1` declares it; the scripts that dropped it produced every wrong reading |
| P3, **not reproduced** | A white band was recorded below the content at the bottom of the window. It does not appear in a DPI-aware capture at 2266x1511: the last rows read luma 24.9 for the tab bar and then 0.0 for the window border, with nothing brighter between. Either `fn force_full_repaint` closed it or it was the same stale-buffer artefact. Left recorded rather than deleted, because one capture at one size is not proof of absence. | capture and scan the bottom rows again after any change to the root layout, with `scripts/stranger-acceptance.ps1` |
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

`scripts/stranger-acceptance.ps1` is the P4 criterion made runnable: it extracts the shipped
ZIP where nothing was built, starts it, and reads the shell header and Today card out of the
accessibility tree while the first import runs. `scripts/measure-archive-cost.ps1` samples
archive growth, journal growth and processor time against the live archive. Both are
operator-run against built artifacts, and both declare DPI awareness before measuring --
without it a capture silently loses half the window, which once produced three recorded
defects that did not exist.

`scripts/validate-p3e-interactive.ps1` belongs here: it takes a built executable and a
built package, so it is an operator-run gate against a packaged artifact, not a CI step.
Its acceptance criteria are in `P3E_ACCEPTANCE.md`. Nothing invokes it automatically, and
nothing should.
