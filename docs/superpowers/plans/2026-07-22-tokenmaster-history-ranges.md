# TokenMaster P3-D Interactive History Ranges Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps
> use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add exact 1/7/30 rolling-day History controls that replace the shared
History/Models/Projects analytics envelope on the existing desktop worker without growing
frontend memory.

**Architecture:** Slint emits a closed preset; `DesktopState` correlates one pending
intent; the controller validates epoch/product/selection generations, maps the preset to
the existing analytics request, and publishes through the current product reducer and
snapshot slot. One terminal notifier rolls back admitted work that ends without a
snapshot. Full refreshes preserve the last successfully published preset.

**Tech Stack:** Rust 1.97, Slint 1.17, bundled SQLite/query facade, PowerShell/Pester
source audits.

**Planning artifact ownership:** Before Task 1, root intentionally commits this plan and
`docs/superpowers/specs/2026-07-22-tokenmaster-history-ranges-design.md` together as
`docs: design bounded shared history ranges`. They are not folded into a worker task.

## Global Constraints

- Exactly three rolling presets: 1, 7, and 30 days; default 30.
- At most 30 History rows/trend bars and the existing Models/Projects caps.
- One existing capacity-one desktop worker; no new worker, thread, timer, cache,
  connection, query owner, or third analytics section.
- One published preset, one persistent scalar high-water generation, one current
  correlation, and one latest pending intent; no range history, queue, or collection.
- Successful selection replaces the one shared recent-usage envelope consumed by History,
  Models, and Projects; Dashboard and Projects UTC-today Git evidence do not change.
- Route selection remains query-free. Only fixed History range callbacks submit work.
- History range and Sessions detail/page work are mutually exclusive at admission; neither
  rejection clears or displaces the already active interaction.
- One optional active Session-detail attempt scalar closes the existing interval after its
  pending slot is consumed; completion clears it on every terminal path.
- Dedicated optional History and Sessions terminal notifier slots cannot both own the same
  attempt and cannot displace each other.
- Intents contain only preset plus epoch/product/checked selection generations; no dates,
  arbitrary counts, scopes, identities, paths, cursors, source content, or query objects.
- Rejection, stale publication, cancellation, deadline, abandonment, and failure leave
  the last successfully published preset selected and clear pending state.
- Range-query failure degrades History, Models, and Projects together while retaining the
  same prior exact shared envelope and selected preset.
- Section-local work publishes a snapshot only for `ProductPublishOutcome::Accepted`;
  only accepted query success changes the preset. Coalesced/older/incompatible reduction
  publishes nothing and terminally rolls back the exact pending intent.
- Every behavior change follows focused RED, observed expected failure, minimal GREEN,
  and focused regression before broader gates.

---

### Task 1: Evolve the normative shared-range contract before behavior

**Files:**
- Modify: `spec/SPECIFICATION.md`
- Modify: `spec/API_CONTRACT.md`
- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `spec/TRACEABILITY.md`

**Interfaces:**
- Produces: the binding rule that one shared recent-usage envelope has a default and
  maximum of 30 days and permits only exact rolling 1/7/30 selection.
- Produces: explicit mutual exclusion and terminal-state rules for History range versus
  Sessions detail/page, refresh, and backend epoch replacement.
- Consumes: approved design
  `docs/superpowers/specs/2026-07-22-tokenmaster-history-ranges-design.md`.

- [ ] **Step 1: Add the normative contract change**

  Replace fixed-30-only MUST language with default-30/shared-1/7/30 language. Record that
  History, Models, and Projects always consume the same selected envelope and all three
  retain/degrade together on range-query failure. Preserve the Dashboard today and
  Projects UTC-today Git ranges.

- [ ] **Step 2: Add the concurrency, anti-replay, privacy, and memory contract**

  Record one persistent scalar high-water generation, one current plus latest pending
  range intent, mutual exclusion with Sessions interactions, the exact arbitration table,
  two non-displacing optional terminal notifier slots, 30-row maximum, and no free-form
  date/count/identity/path/query state in the new control boundary.

- [ ] **Step 3: Self-review normative consistency**

  Search all six documents for contradictory fixed-30-only claims, unstated Models/
  Projects behavior, or accidental release claims. `UsageRange::recent_days(30)` remains
  the default, not the only legal shared request. Run `git diff --check`.

- [ ] **Step 4: Commit the contract**

  Commit as `docs: define bounded shared history ranges`. No production behavior may
  change before this commit.

### Task 2: Define and execute one constant-state History range intent

**Files:**
- Modify: `crates/desktop/src/controller.rs`
- Modify: `crates/desktop/src/lib.rs`
- Create: `crates/desktop/tests/history_range_controller_contract.rs`
- Modify: `crates/desktop/tests/controller_contract.rs`

**Interfaces:**
- Produces: `DesktopHistoryRangePreset::{Recent1Day, Recent7Days, Recent30Days}` with
  `day_count() -> u16` and `stable_code() -> &'static str`.
- Produces: checked `DesktopHistoryRangeGeneration::new(u64)` and
  `DesktopHistoryRangeIntent::new(DesktopSnapshotEpoch, ProductGeneration,
  DesktopHistoryRangeGeneration, DesktopHistoryRangePreset)`.
- Produces: `DesktopController::request_history_range(intent) ->
  Result<DesktopRefreshAdmission, DesktopControllerError>`.
- Produces: `DesktopTerminalHistoryRangeNotifier::history_range_terminal(intent)` and
  `DesktopController::attach_terminal_history_range_notifier(...)`.
- Consumes: existing `UsageRange::recent_days`, `UsageAnalyticsRequest`,
  `ProductReducer::{publish_history,fail_history}`, worker/publication primitives.

- [ ] **Step 1: Write RED value/admission tests**

  Add tests proving the enum maps only to 1/7/30, generation zero is rejected, default
  plan is 30 days, same-preset/stale epoch/stale product/non-newer generation admissions
  reject, and the public debug/type surface contains no arbitrary date/count/scope/path.

- [ ] **Step 2: Run the RED tests and record the expected missing-symbol failures**

  Run:

  ```powershell
  cargo +1.97.0 test -p tokenmaster-desktop --test history_range_controller_contract --locked
  ```

  Expected: compile failure for the new History range types/methods, not a fixture or
  dependency error.

- [ ] **Step 3: Implement the minimal typed controller state**

  Add exactly these constant-capacity fields to `DesktopWorkState`: published preset
  (initialized to 30), optional current range work, optional pending range work, and a
  persistent monotonic high-water generation, plus one optional active Session-detail
  attempt scalar needed only for cross-interaction exclusion after its pending slot is
  consumed. Extend `DesktopWorkBatch` with one optional intent. Do not add `Vec`, map,
  channel, range request collection, or second worker.

- [ ] **Step 4: Add RED scheduling/publication tests**

  Prove 1/7/30 requests preserve system timezone, daily series, and Model/Project
  breakdowns; 10,000 submissions retain only latest eligible work; success replaces the
  shared History payload and advances the published preset; query failure retains the
  prior payload/preset; stale epoch/product/range generation cannot publish; full refresh
  uses the last successful preset and supersedes admitted range work. Add a table-driven
  RED contract proving range and Sessions detail/page admissions are mutually exclusive in
  both directions without clearing the pre-existing interaction, including the interval
  while a Session-detail query is executing after its pending slot has been consumed. Use
  a blocking source fixture to submit the range after exact detail-query entry and before
  release; assert `Busy` and unchanged detail/range correlations.

- [ ] **Step 5: Run RED and implement minimal worker execution**

  Build the request only inside the controller from the closed preset. Execute range work
  before ordinary refresh only when its exact work-attempt correlation is current. Validate
  epoch/product/generation before query and while holding the commit correlation. Reject
  range while Sessions detail/page is current or pending, and reject those Sessions paths
  while range work is current or pending. On
  success call `publish_history`; only `ProductPublishOutcome::Accepted` may atomically
  update the published preset and publish the snapshot. On query error call `fail_history`,
  keep the prior preset, and publish the degraded snapshot only when that outcome is
  `Accepted`. For `Coalesced`, `RejectedOlder`, or `RejectedIncompatible`, publish no
  snapshot, keep the preset/correlation unchanged, and let terminal completion roll back
  the exact intent. Add RED fixtures for all non-accepted outcomes, including dataset
  identity drift. Refresh reads the published preset and invalidates current/pending range
  work.

- [ ] **Step 6: Add and verify terminal recovery**

  Add RED tests for cancellation, deadline, pending-deadline, abandoned follow-up, and
  refresh supersession. The single typed notifier receives the exact still-current intent
  only when no snapshot commit consumed it. Verify snapshot publication precedes terminal
  callback observation and terminal rollback is idempotent. Prove the dedicated optional
  History and Sessions terminal slots cannot both own the same attempt or displace each
  other.

- [ ] **Step 7: Run focused GREEN and strict package checks**

  ```powershell
  cargo +1.97.0 test -p tokenmaster-desktop --test history_range_controller_contract --locked
  cargo +1.97.0 test -p tokenmaster-desktop --test controller_contract --locked
  $env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy -p tokenmaster-desktop --all-targets --locked
  ```

  Expected: all pass with no warnings. Commit as `feat(desktop): add bounded history range work`.

### Task 3: Correlate range state through presentation, bridge, and application

**Files:**
- Modify: `crates/desktop/src/history.rs`
- Modify: `crates/desktop/src/presentation.rs`
- Modify: `crates/desktop/src/bridge.rs`
- Modify: `crates/desktop/src/ui.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/app/src/application.rs`
- Modify: `crates/app/src/application_tests.rs`
- Modify: `crates/desktop/tests/history_projection_contract.rs`
- Create: `crates/desktop/tests/history_range_bridge_contract.rs`

**Interfaces:**
- Consumes: Task 2 preset/intent/controller/notifier types.
- Produces: `DesktopState::request_history_range(preset) ->
  Result<DesktopHistoryRangeIntent, DesktopHistoryRangeSelectionError>` and exact
  `reject_history_range(intent)` / `complete_history_range_terminal(intent)` rollback.
- Produces: `DesktopHistoryProjection::{range_preset(),range_pending()}`.
- Produces: `DesktopHistoryRangeIntentSink` with accepted/rejected admission and an
  unavailable implementation.

- [ ] **Step 1: Write projection/state RED tests**

  Prove initial waiting state selects 30 and is not pending; exact 1/7/30 daily series
  select the corresponding preset; other series lengths fail closed to the last/default
  selection; requesting the already published preset or a second pending selection is
  rejected; accepted input sets only one pending correlation; newer snapshot, exact
  rejection, terminal rollback, and epoch replacement clear it.

- [ ] **Step 2: Run RED and implement minimal presentation state**

  ```powershell
  cargo +1.97.0 test -p tokenmaster-desktop --test history_projection_contract --locked
  ```

  Expected first run: compile failures for new projection/state APIs. Implement preset
  derivation from the exact 1/7/30 daily series length, one active intent, checked next
  generation, and synchronous pending projection. Do not store request/range history.

- [ ] **Step 3: Write bridge/application RED tests**

  Prove weak-window terminal delivery rolls back only the exact active intent after any
  already queued snapshot is applied; application routing is nonblocking under bundle
  contention; missing/safe-mode/stale/closed controllers reject; backend epoch replacement
  cannot accept an old intent; controller attachment occurs exactly once.

- [ ] **Step 4: Implement the minimal routing and rollback path**

  Add one range sink/router in the existing application bundle, one bridge notifier using
  `slint::invoke_from_event_loop`, and exact projection application after admission,
  rejection, or terminal completion. Keep locks out of application-to-controller calls in
  the same pattern as Sessions navigation.

- [ ] **Step 5: Verify focused GREEN and regressions**

  ```powershell
  cargo +1.97.0 test -p tokenmaster-desktop --test history_projection_contract --locked
  cargo +1.97.0 test -p tokenmaster-desktop --test history_range_bridge_contract --locked
  cargo +1.97.0 test -p tokenmaster-app application_tests --locked
  cargo +1.97.0 test -p tokenmaster-desktop --test bridge_event_loop_contract --locked
  $env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy -p tokenmaster-desktop -p tokenmaster-app --all-targets --locked
  ```

  Expected: all pass with no warnings. Commit as `feat(app): route history range intents`.

### Task 4: Add accessible fixed range controls without model growth

**Files:**
- Modify: `crates/desktop/ui/views/history-view.slint`
- Modify: `crates/desktop/ui/main.slint`
- Modify: `crates/desktop/src/ui.rs`
- Modify: `crates/desktop/tests/ui_contract.rs`

**Interfaces:**
- Consumes: Task 3 projection and sink.
- Produces: Slint properties `history-range-preset`, `history-range-pending` and callbacks
  `request-history-range-1`, `request-history-range-7`, `request-history-range-30`.

- [ ] **Step 1: Write live/structural Slint RED tests**

  Assert exact `1 day`, `7 days`, `30 days` labels and accessible names; 30 is initially
  selected; current selection is inert; accepted input sets pending synchronously and
  disables all controls; rejected/terminal work restores them; pointer, Enter, Space, and
  Tab focus work; route-only switching emits no range intent and keeps row model identity.

- [ ] **Step 2: Run RED and implement minimal UI wiring**

  ```powershell
  cargo +1.97.0 test -p tokenmaster-desktop --test ui_contract --locked history_range
  ```

  Expected first run: missing Slint properties/callbacks or failed structural assertions.
  Add one compact control row to the History header, standard focusable buttons, fixed
  callbacks, and a `wire_history_range_intents` binding following the exact reject-safe
  Sessions pattern.

- [ ] **Step 3: Prove replace-only bounded memory behavior**

  Add 10,000 accepted snapshot replacements across 1/7/30 and assert the single History,
  Models, and Projects models do not accumulate old rows or range-selection state. Assert
  History never exceeds 30 rows/trend bars and no append/load-more path exists.

- [ ] **Step 4: Verify GREEN**

  ```powershell
  cargo +1.97.0 test -p tokenmaster-desktop --test ui_contract --locked
  cargo +1.97.0 test -p tokenmaster-desktop --test history_projection_contract --locked
  cargo +1.97.0 test -p tokenmaster-desktop --locked
  $env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy -p tokenmaster-desktop --all-targets --locked
  ```

  Expected: all pass with no warnings. Commit as `feat(ui): add bounded history range controls`.

### Task 5: Pin executable authority and regression audits

**Files:**
- Modify: `scripts/audit-desktop-shell.ps1`
- Modify: `scripts/audit-application-composition.ps1`
- Modify: `scripts/tests/audit-desktop-shell.Tests.ps1`
- Modify: `scripts/tests/audit-application-composition.Tests.ps1`

**Interfaces:**
- Consumes: Tasks 2-4 production topology.
- Produces: mutation-resistant source gates for fixed presets, one worker/slot, stale
  fences, refresh precedence, terminal/application wiring, shared-range replacement,
  30-row bound, and forbidden authority/state surfaces.

- [ ] **Step 1: Add RED mutations before audit implementation**

  Each mutation must make the current audit pass first, then demonstrate the missing gate:
  arbitrary count/date input, fourth preset, range `Vec`/map/queue, extra worker/query
  owner, removed epoch/product/generation fence, full-refresh reset to 30, stale terminal
  rollback, absent application attachment, route callback query, 31-row projection, and
  separated/duplicate Models or Projects analytics call.

- [ ] **Step 2: Run RED Pester targets**

  ```powershell
  Invoke-Pester scripts/tests/audit-desktop-shell.Tests.ps1 -Output Detailed
  Invoke-Pester scripts/tests/audit-application-composition.Tests.ps1 -Output Detailed
  ```

  Expected: only new mutations fail because the audit lacks the new rejection.

- [ ] **Step 3: Implement exact executable audits**

  Replace obsolete fixed-request assertions with executable-body checks that allow only
  the designed section-local range request while preserving exactly two analytics calls
  in full refresh. Avoid comment/cfg/unreachable false authority. Keep failure codes stable
  and slice-specific.

- [ ] **Step 4: Verify focused audits and production composition**

  ```powershell
  Invoke-Pester scripts/tests/audit-desktop-shell.Tests.ps1 -Output Detailed
  Invoke-Pester scripts/tests/audit-application-composition.Tests.ps1 -Output Detailed
  pwsh -NoProfile -File scripts/audit-desktop-shell.ps1 -RepositoryRoot (Get-Location).Path -Configuration release
  pwsh -NoProfile -File scripts/audit-application-composition.ps1 -RepositoryRoot (Get-Location).Path -Configuration release
  pwsh -NoProfile -File scripts/audit-product-status.ps1 -RepositoryRoot (Get-Location).Path
  ```

  Expected: all pass. Commit as `test(audit): pin bounded history ranges`.

### Task 6: Synchronize project state and close verification

**Files:**
- Modify: `spec/TRACEABILITY.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/AUDIT_AND_MASTER_PLAN.md`
- Modify: `docs/FEATURE_PARITY.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: `docs/CHANGELOG.md`

**Interfaces:**
- Consumes: verified behavior and exact gate counts/timings from Tasks 1-5.
- Produces: one consistent durable continuation rail with no tracked current commit hash.

- [ ] **Step 1: Reconcile normative contracts with implemented evidence**

  Verify Task 1 wording still exactly matches implemented behavior and focused evidence;
  update traceability status without weakening the 1/7/30, shared-envelope, rollback,
  fencing, memory, or privacy contract. Do not claim P4/P5/P6/M0/package/signing/soak/
  release acceptance.

- [ ] **Step 2: Update state, traceability, parity, history, and handoff**

  Record actual focused counts and review findings only after they exist. Name the next
  genuinely open roadmap slice. Do not store the current commit hash in tracked docs.

- [ ] **Step 3: Run focused/full product gates**

  ```powershell
  cargo +1.97.0 test -p tokenmaster-product --locked
  cargo +1.97.0 test -p tokenmaster-desktop --locked
  cargo +1.97.0 test -p tokenmaster-app --locked
  pwsh -NoProfile -File scripts/audit-product-status.ps1 -RepositoryRoot (Get-Location).Path
  pwsh -NoProfile -File scripts/audit-desktop-shell.ps1 -RepositoryRoot (Get-Location).Path -Configuration release
  pwsh -NoProfile -File scripts/audit-application-composition.ps1 -RepositoryRoot (Get-Location).Path -Configuration release
  ```

- [ ] **Step 4: Request independent Sol High review and resolve findings with RED/green**

  Review correctness, concurrency, stale publication, failure recovery, shared-envelope
  semantics, privacy, memory, audits, and documentation. Any Critical/Important finding
  receives a focused failing regression before the fix and an independent re-review.

- [ ] **Step 5: Run the exact baseline**

  ```powershell
  pwsh -NoProfile -File scripts/audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
  cargo +1.97.0 fmt --all -- --check
  $env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
  cargo +1.97.0 test --workspace --locked
  ```

  Expected: exit 0 for every gate; record durations and any dependency-only warnings.

- [ ] **Step 6: Audit and clean task-owned state**

  Confirm branch/HEAD/status, close all task agents, stop only task-owned processes, remove
  only task-owned temporary reports/fixtures, and remove heavy build artifacts only after
  all verification is complete. Commit verified documentation as
  `docs: close bounded history ranges`.
