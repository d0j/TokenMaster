# TokenMaster release exit plan

The per-cycle delivery rail until the first accepted release. Repository contracts and
acceptance documents remain authoritative.

## Current milestone

One downloadable, deterministic, unsigned Windows x64 portable `0.1.0` under Apache-2.0.
Not a stable release and not signed.

Acceptance is a stranger, not a receipt: a machine that has never built this repository
downloads the packaged executable, launches it, and sees its own Codex history with a
lifetime total and a quota board that states its evidence honestly -- filled where the
runtime could reach Codex, and degraded with a named reason where it could not.

**The quota clause was narrowed deliberately, and the price is written down.** It read "a
working quota board", which this product cannot promise on a machine whose Codex is an npm
install, because every mechanism that would find the binary there is closed by a contract
this repository already holds. The entry below carries the measurement; the reason the
narrowing is legitimate rather than convenient is that the board no longer lies about it.

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

### P4, measured against the shipped package

Run against `dist/TokenMaster-0.1.0-windows-x64-unsigned.zip` built at the commit it
reports, extracted where nothing was built, with `scripts/stranger-acceptance.ps1`. These
are the numbers the `v0.1.0` tag carries, from the run against that tag's own package --
an earlier run's figures stood here until the tag was cut, which is how evidence and the
thing it certifies drift apart.

- **The package is the commit.** Its receipt commit equals `HEAD`, and
  `validate-product-package.ps1` passes. Rebuild before tagging or the receipt stops
  matching.
- **A stranger sees their own history.** `Tokens 335,518,571` and `Events 2,319 events`,
  identical at 60, 120 and 180 seconds -- filled and never falling back to dashes.
- **The lifetime total is live.** `29,335,964` at launch, then 4.60, 11.78 and 11.92 billion
  as the import runs.
- **An observed empty day reads as a zero, not as missing.** `Tokens 0` at launch, beside
  `Events 0 events`.
- **The quota board states its evidence honestly.** `Degraded: quota_discovery` at all four
  marks, where it previously showed a green `Ready` with no reason beside
  `Quota evidence unavailable`.
- **Runtime libraries come from the package**, all three, with nothing borrowed from
  System32.

**The script can now fail on the clause it exists to guard, and could not before.** It read
the header and the Today card and never looked at the state pill, so every earlier run
reported a pass while the card claimed a finished answer. It now refuses exactly one
combination -- `Ready` beside a board saying it has no evidence -- and that decision was
exercised with both states actually observed here, the pre-fix `Ready` and the post-fix
`Degraded: quota_discovery`, plus the filled-board case a machine with Codex would produce.

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
| **P4, decided** | **No location this code could guess would fix the quota board, and every non-guessing route is closed by a contract this repository already holds.** Quota is not read from the archive; the runtime spawns `codex.exe app-server --stdio`, discovery reads `PATH` and nothing else, and `codex` on `PATH` here is only the npm shims. Four `codex.exe` exist at three versions: `%LOCALAPPDATA%\OpenAI\Codex\bin` holds `0.130.0-alpha.5`, `~/.codex/.sandbox-bin` and `~/.codex/plugins/.plugin-appserver` hold `0.146.0-alpha.3.1`, and the npm package vendors `0.144.1`, which is what `codex --version` answers. A fix searching the `%LOCALAPPDATA%` directory was written, pinned by contract and **reverted** -- it selects `0.130.0-alpha.5`, on which `codex login status` fails with `invalid type: map, expected a string`, so the guess lands on the one binary that cannot work. Following the shim means running `node` with `codex.js`, which the documented script and package-wrapper rejection forbids. Configuration is what is left, and its price was measured rather than assumed: `spec/API_CONTRACT.md` states `ApplicationEnvironment` captures only the executable, `LOCALAPPDATA`, `USERPROFILE` and `CODEX_HOME` and that production accepts no general environment-selected path, so an environment variable contradicts a binding spec; the settings profile holds **no string or path field at all**, so a persisted path means `SETTINGS_SCHEMA_VERSION` 7 to 8 through a hand-written ladder with frozen wire snapshots and three production call sites of `PortableSettings::new` that would otherwise reset it silently; `spec/SECURITY.md` forbids retaining absolute paths in exported surfaces, so it could not live in the portable half; and one new Settings label moves a closed msgid array, the literal `518` in ten assertions, and two hand-edited catalogs. **Decision: not before the tag.** The plan's own abort rule says a migration ladder never holds the first release hostage, and the criterion is narrowed instead -- legitimate only because the board now names why it has nothing, which is the entry below. | `fn resolve_current_command` in `crates/runtime/src/quota/config.rs`; `SETTINGS_SCHEMA_VERSION` in `crates/state/src/settings/value.rs` |
| **closed** | **The Plan Usage card claimed a finished answer to a question that was never successfully asked.** On a live window it rendered a `Ready` pill with no reason codes beside `Quota evidence unavailable`. Quota is the one Dashboard figure that does not come from the archive -- the runtime spawns Codex and asks it -- and the archive query succeeds either way, finding no windows, so the section reported Ready on a query that answers nothing about whether the question was ever asked. **The first fix was incomplete and the live window caught it.** A failed poll arrives by two routes: the runtime may be unobservable, which is the observation error, or it may be perfectly observable and reporting that its last poll failed, which is the health's own `quota_failure`. A discovery failure -- no executable found at all, which is this machine -- takes the second route, and the desktop crate did not read that field anywhere. Reading only the first route passed its own test and left the card unchanged in front of a user. Both routes are read now. Each half was red first (`left: Ready, right: Ready`, then `expected quota_transport, received []`) and each was verified by removing only that half and watching its own assertion fail again. | `fn map_plan_usage` in `crates/desktop/src/dashboard.rs`; `a_failed_quota_poll_reaches_the_card_that_reports_quota` in `crates/app/tests` |
| **closed** | The Today card reported `Cost $0.00` beside `Tokens --` for the same empty day. `AggregateTokenValue::from_store` asked `known_count == 0` before `known_count == event_count`, and that question cannot tell an observed empty day apart from a day whose events carry no token evidence -- only `event_count` separates them. The section is `Ready` in this state, so the answer is complete rather than mid-import, and the event count and the cost both already called the zero known: two of three agreed and only tokens dissented. `event_count == 0` is now asked first and answers `Known(0)`. A day with events but no token evidence still answers `Unavailable`, and a section with no answer yet is still `Waiting` reporting `None`. The contract that pinned the old asymmetry was rewritten rather than deleted, and the release criterion that an absent value and a measured zero must not render alike is untouched -- `Known(0)` formats as `0`, unavailable as an em dash, and that pairing is pinned by its own test. Red first: `left: Unavailable, right: Known(0)`. | `fn from_store` for `AggregateTokenValue` in `crates/query/src/analytics.rs`; `an_observed_empty_day_is_a_known_zero_and_missing_evidence_is_not` |
| post-tag | **The designer delivered a full handoff and it is now the standard, superseding both earlier drawings.** `docs/design/` holds the specification, the 1400x900 target, two 2x captures and the source app it re-composes; `docs/UI_REFERENCE.md` no longer transcribes it, it measures the distance to it. That distance, checked against the code rather than estimated: the sidebar is six items plus Settings and Help while `ProductRoute::ALL` is eleven, so History, Notifications and Compact Widget have no drawn place -- a product decision, not a drawing one. The window is fixed at 1400x900 with **nothing scrolling** and only the trend row elastic, against a Dashboard that wraps its content in a `ScrollView` sized `max(self.height, content.min-height)`. Three cards do not exist at all: Live Sessions with branch, context percentage and tool-call chips; an Activity heatmap of 7 days by 24 hours against the product's eight rows of something else; and a continuous 1-to-90-day SCALE slider that re-scopes the trend. **The heatmap's grid does not exist, and this entry said it did.** `USAGE_RHYTHM_HOURS` is 24 and `USAGE_RHYTHM_WEEKDAYS` is 7, but they bound two marginals -- `UsageRhythm { hours, weekdays }` is thirty-one buckets, not a hundred and sixty-eight, and the busiest hour beside the busiest weekday does not say Tuesday at 03:00. The inference from two true constants to a grid was mine and went into two documents before the type was read. The history request does ask for rhythm over thirty days, so the query path and its bounds are there; the missing piece is an hour-of-weekday bucket -- and that is smaller than it sounds, because `rhythm_segment` in `rhythm_sql` already carries `hour_index` and `weekday_index` on every row and the query simply groups by one of them at a time. A joint grid is a key series of 168 and a join on `weekday_index * 24 + hour_index`, not a schema change. Two font families must be bundled. Every number in the handoff is mock data: the layout is the specification and the values come from the archive. | `docs/design/README.md`; `docs/UI_REFERENCE.md`, its "distance from here" section |
| post-tag | **Let the user say where their Codex is.** The decision above defers this, not cancels it: `CodexQuotaRuntimeConfig::with_executable` already accepts an explicit path, already wins over discovery, and already validates it -- exact `.exe`, no reparse point, canonicalised, no shim -- and an invalid one is refused without falling back, which a contract pins. Only a way to set it is missing. The shape the code already supports: one accessor on `ApplicationEnvironment` and one parameter on `finish_live_bundle`, whose four callers all hold the environment in scope, with `OptionalRuntime::start` absorbing a bad path into a degraded runtime rather than a failed launch. What makes it post-tag is everything around that seam -- a settings schema step, a first-ever string field, a security invariant about retained absolute paths, and a closed localization set -- all costed in the entry above. | `fn with_executable` in `crates/runtime/src/quota/config.rs`; `fn finish_live_bundle` in `crates/app/src/application.rs` |
| post-tag | **A resource contract fails when the machine is busy, which makes it a gate that can be talked out of.** `resource_contract` in `crates/query` demands a topology-stable retained plateau within 64 rounds and reads private bytes to find it. Run beside a packaged build and a DPI capture it saw private bytes swinging between 7.20 MB and 3.40-4.61 MB while handles, threads and user objects never moved, and gave up: `did not establish a topology-stable retained plateau within 64 rounds`. Re-run alone on the same commit it passes. The measurement is therefore load-sensitive rather than wrong, and that is the problem: a check that fails for a reason unrelated to the code teaches its reader to dismiss it, and the next dismissal will be of a real regression. Handles and object counts held perfectly steady throughout, so the plateau condition is the part to reconsider -- either bound it to the counters that are actually stable, or make the byte plateau tolerant of an allocator that has not returned pages yet. | `resource_contract` in `crates/query/tests`; the plateau loop that reports `samples=[...]` |
| **closed** | **The localization contract checked one direction only, and a shipped string was already through the hole.** It asserts that every entry of its closed msgid arrays appears as `@tr(...)` in the Slint tree. There is no assertion the other way -- that every `@tr` in the tree is in an array -- so a label added to a view and forgotten in the catalog passes. One already has: `@tr("Alerts")` renders in the shell tab strip while `msgid "Alerts"` exists in neither `translations/ru` nor any contract array, which means it ships as English in Russian today. The reverse assertion now walks the `ui` tree from disk -- from disk, because a list of sources would have to be remembered and forgetting is the failure it exists to catch -- and requires every `@tr` to have an entry in both catalogues. Red first with exactly one name, then green, then verified by deleting the entry again. Two guards stop it degrading: fewer than eleven sources or fewer than a hundred literals is itself a failure. One correction on the way, recorded in the code: the catalogues are CRLF, so a match anchored to a bare newline named all 583 literals as missing, and a test that cries about everything teaches its reader to skip it. | `crates/desktop/tests/localization_contract.rs`; `@tr("Alerts")` in `crates/desktop/ui/main.slint` |
| post-tag | **Two font families the design requires are absent, and the type token points at neither.** The handoff specifies JetBrains Mono for every number and technical label and IBM Plex Sans for navigation and prose, both bundled for an offline build. `UiTokens.mono` is `"Consolas"`, there is no proportional-family token at all, and nothing in the crate registers a font. Every size in the spec's scale also collides with the existing density-scaled steps, and the fifteen colour roles in `UiPalette` cannot express the spec's separate chrome, rail, hero and inset surfaces or its six border tones. **The design is not reachable by restyling; it needs the token system widened first.** | `UiTokens.mono` in `crates/desktop/ui/tokens.slint`; `fn ui_palette` in `crates/desktop/src/skin.rs` |
| **closed** | **Eight Activity labels shipped untranslated in every locale, outside the contract by construction.** `activity_label` in `crates/desktop/src/ui.rs` returns `Read`, `Edit / Write`, `Search`, `Git`, `Build / Test`, `Web`, `Subagents` and `Terminal` as bare `&'static str` from Rust. The contract asserts a closed msgid set and that each entry appears as `@tr(...)` inside the Slint tree -- so a string emitted from Rust escapes it **by construction**, not by oversight. That is the same shape as three other findings this cycle: a gate of which only a third was run, an acceptance script that could not fail on its own clause, and a resource check that fails for reasons unrelated to the code. Fixed by removing the English path rather than wrapping it: the helper is gone, the `label` field is gone from `DashboardActivityRow`, and the card resolves the stable key through `ProjectionStrings` like every other keyed string. Wrapping would have fixed the eight and left the next category free to arrive the same way. The reverse assertion above now covers them, since they are `@tr` in the tree. | `fn activity_label` in `crates/desktop/src/ui.rs`; the closed-set assertions in `crates/desktop/tests/localization_contract.rs` |
| post-tag | **Live Sessions asks for facts the whole workspace does not hold.** Mapped crate by crate rather than guessed: the branch name is parsed by the Codex reader and rides only inside an opaque resume blob, with no column in `usage_observation` or `usage_event` and no query accessor. Context percentage, tokens remaining and "at limit" exist nowhere -- `context_window` appears only in the parser and is the model's window size, not consumption. Per-tool names and counts live in `ParserState::tool_counts` and never leave `crates/codex`; the store keeps eight aggregate activity columns instead. Session liveness has no concept in any crate. The originator is persisted but the query layer never exposes it, and there is no session-to-repository join at all. **So this card is ingestion and schema work before it is UI work**, and it is the one part of the handoff that cannot be reached by carrying existing data. | `ParserState::tool_counts` in `crates/codex/src/parser/state.rs`; `usage_event` columns in `crates/store/src/usage/schema.rs` |
| **next, wrong number on screen** | **The Dashboard's cost per hundred added lines is arithmetically wrong whenever a project has more than one repository.** `map_code_output` accumulates `efficiency_usage.checked_add(value.usage_cost().get())` over every repository, but that cost is the **project's**, not the repository's: `map_repository` fills it from `projects[project_index].1`, so two repositories sharing an alias carry the identical value and summing them multiplies it. The Projects route knows this and refuses -- `ProjectEfficiencyAccumulator::add_available` raises `efficiency_evidence_mismatch` if two costs differ and never sums them, adding only lines. So the two screens report different numbers for the same repositories, and the Dashboard's is N times too large for a project of N repositories. `map_code_output` also ignores `usage_dataset_identity`, blending evidence across datasets where the Projects route refuses to. Both contract fixtures use a single repository, so the fan-out is uncovered. **The query layer already computes the correct figure** as `GitEfficiencyValue::cost_per_100_added_lines`, and it has no production consumer at all -- only a test reads it. | `fn map_code_output` in `crates/desktop/src/dashboard.rs`; `ProjectEfficiencyAccumulator::add_available` in `crates/desktop/src/projects.rs` |
| post-tag | **The trend chart's markers do not sit on its line.** The polyline is emitted from Rust into a 1000x100 viewbox with `x = 28 + i/(n-1) * 944` and a band from y 10 to 80, while the dots, hover crosshair, tooltip anchor and axis labels are placed by a Slint overlay at the centre of an equal-stretch cell, `(i + 0.5)/n`, over a band of `parent.height - 26px`. Two independent statements of where point `i` sits, in different coordinate systems: about one percent of chart width apart at both ends and several pixels apart vertically, so the crosshair annotates the wrong column at the edges. A third statement of the same geometry is `TREND_ZERO_LINE = 80.0`, written as a literal where it is `10.0 + 70.0` from the y formula, so editing the band silently detaches every area fill from its line. | `fn dashboard_trend_path` and `const TREND_ZERO_LINE` in `crates/desktop/src/ui.rs`; the marker overlay in `dashboard-view.slint` |
| post-tag | **The reminder preset lead set is written three times in one file** -- a function-local `PRESETS` array, five separate `leads.contains` calls, and a fourth literal list in the custom-row filter. Disagree and a lead is shown both as a ticked preset and as an editable row, where `dedup` on save silently reverts the user's edit, or it is dropped from both and lost. A coupled sub-instance: `reminder_custom_rows` chooses days whenever the lead divides by 86,400 while `format_reminder_lead` requires at least two days, so one value renders as `1 day` in the editor and `24h` in the summary. Unreachable today only because the preset filter strips it. | `fn reminder_policy_intent` and `fn apply_reliable_state` in `crates/desktop/src/ui.rs` |
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
