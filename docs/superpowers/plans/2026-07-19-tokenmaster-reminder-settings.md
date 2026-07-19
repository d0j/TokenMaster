# TokenMaster Reminder Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the portable global reminder profile editable, generation-bound to the
effective benefit archive, visibly synchronized, and bounded in the responsive Settings UI.

**Architecture:** Reliable settings remain desired-state authority. A store-owned
transaction projects settings generation into the global reminder profile while
preserving scope overrides. The existing single app operation worker persists and
synchronizes; Desktop receives one copyable fixed-cap projection plus one typed bounded
intent and keeps only an eight-row draft model.

**Tech Stack:** Rust 1.97.0, Slint 1.17.1 software renderer, bundled SQLite through
rusqlite, PowerShell/Pester computed audits.

## Global Constraints

- One root Cargo workspace; no dependency or workspace addition.
- One portable global profile; per-scope editing is not part of this slice.
- Enabled means 1..=8 unique leads; disabled means zero leads.
- Every lead is 60..=31,536,000 seconds and is stored descending.
- Settings generation `N` maps to global profile revision `N + 1`; defaults map to 1.
- Global edits never rewrite scope overrides or delivery/acknowledgement rows.
- Store mutation sees at most 32 inheriting scopes and 256 current lots.
- Slint retains exactly five preset flags and eight custom rows; no text parser.
- No new worker, timer, polling loop, queue, path, identity, SQL, runtime, or settings
  authority crosses into Desktop/Slint.
- Durable settings success with archive projection failure is visible `pending`, not a
  false rollback or false synchronized state.
- Snooze, quiet hours, OS/tray delivery, usage alerts, activation, P4/P5/P6, M0,
  packaging, signing, soak, and release remain incomplete.

---

### Task 1: Atomic global profile projection in the store

**Files:**
- Modify: `crates/store/src/usage/benefit_write.rs`
- Modify: `crates/store/tests/benefit_write_contract.rs`
- Modify: `crates/store/src/usage/benefit_types.rs` only if the existing result needs
  a checked total-count helper

**Interfaces:**
- Consumes: `ReminderProfile`, existing benefit scope/current-lot/profile tables.
- Produces:
  `UsageStore::set_benefit_reminder_global_profile(&ReminderProfile) -> Result<BenefitProfileApplyResult, StoreError>`.

- [ ] **Step 1: Write failing global replacement tests**

Add real SQLite tests with two scopes. Scope A inherits global; scope B has an override.
Assert global revision 2 with `[21_600, 10_800]` rebuilds only A, B remains exact,
delivery rows survive, and reopening returns the same values.

```rust
let applied = store
    .set_benefit_reminder_global_profile(&profile(2, &[21_600, 10_800], true))
    .expect("global profile");
assert_eq!(applied.pending_due_count(), 4);
```

- [ ] **Step 2: Run RED**

Run:
`cargo +1.97.0 test -p tokenmaster-store --test benefit_write_contract global_profile --locked`

Expected: compile failure because `set_benefit_reminder_global_profile` does not exist.

- [ ] **Step 3: Implement the bounded transaction**

Implement a one-lookahead loader that validates scope hash and total-lot caps before
mutation, then use this exact revision admission:

```rust
match profile.revision().get().cmp(&current.revision().get()) {
    Ordering::Less => return Err(StoreError::new(StoreErrorCode::StaleRevision)),
    Ordering::Equal if profile == &current => return current_result(transaction, global),
    Ordering::Equal => return Err(StoreError::new(StoreErrorCode::InvalidValue)),
    Ordering::Greater => {}
}
```

Replace only the global row/thresholds; delete and rebuild due rows only for inherited
scopes. Use `checked_replace_count`, checked total-lot/count conversions, and one commit.

- [ ] **Step 4: Add RED edge tests and make GREEN**

Cover identical no-op, stale revision, same-revision equivocation, disabled empty
profile, 33-scope lookahead, 257-lot capacity, and injected SQL failure rollback.

Run:
`cargo +1.97.0 test -p tokenmaster-store --test benefit_write_contract --locked`

Expected: all tests pass; no profile/due/delivery partial state after rejection.

- [ ] **Step 5: Commit**

```powershell
git add -- crates/store/src/usage/benefit_write.rs crates/store/tests/benefit_write_contract.rs crates/store/src/usage/benefit_types.rs
git diff --cached --check
git commit -m "feat(store): synchronize global reminder profile"
```

### Task 2: Fixed Desktop policy projection and typed intent

**Files:**
- Modify: `crates/desktop/src/reliable_state.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/desktop/tests/reliable_state_projection_contract.rs`

**Interfaces:**
- Produces `DesktopReminderSyncState::{Pending,Synchronized,Unavailable}`.
- Produces `DesktopReminderPolicy::new(bool, &[u32], DesktopReminderSyncState) -> Option<Self>`.
- Produces `DesktopIntent::update_reminder_policy(bool, &[u32]) -> Result<Self, DesktopIntentValidationError>`.

- [ ] **Step 1: Write failing projection/intent tests**

Prove descending exact values, unavailable fallback, duplicate/bound/cap rejection,
enabled-empty/disabled-nonempty rejection, and redacted `Debug`.

```rust
let policy = DesktopReminderPolicy::new(
    true,
    &[604_800, 10_800],
    DesktopReminderSyncState::Synchronized,
).expect("policy");
assert_eq!(policy.lead_seconds(), &[604_800, 10_800]);
assert!(!format!("{:?}", DesktopIntent::update_reminder_policy(true, &[10_800])?).contains("10800"));
```

- [ ] **Step 2: Run RED**

Run:
`cargo +1.97.0 test -p tokenmaster-desktop --test reliable_state_projection_contract --locked`

Expected: compile failure for missing policy/sync types.

- [ ] **Step 3: Implement fixed-cap values**

Store leads as `[u32; 8]` plus `u8` count. Keep `DesktopReliableStateSummary` copyable,
add the policy field/constructor parameter/getter, and add a disabled-unavailable
fallback. Implement typed intent validation before allocating the boxed slice.

- [ ] **Step 4: Run GREEN**

Run:
`cargo +1.97.0 test -p tokenmaster-desktop --test reliable_state_projection_contract --locked`

Expected: all projection tests pass.

- [ ] **Step 5: Commit**

```powershell
git add -- crates/desktop/src/reliable_state.rs crates/desktop/src/lib.rs crates/desktop/tests/reliable_state_projection_contract.rs
git diff --cached --check
git commit -m "feat(desktop): project reminder settings"
```

### Task 3: Persist and synchronize settings in application state

**Files:**
- Modify: `crates/app/src/state.rs`
- Modify: `crates/app/src/state_tests.rs`

**Interfaces:**
- Produces `ApplicationStateOwner::update_reminder_policy(...) -> Result<(), ApplicationError>`.
- Produces `ApplicationStateOwner::synchronize_reminder_profile(&DataRoot) -> Result<ReminderProfile, ApplicationError>`.
- Produces constant-size `reminder_sync_state` included in reliable projection.

- [ ] **Step 1: Write RED state tests**

Use real redundant settings records plus real SQLite. Prove first explicit Save creates
one generation, identical retry creates no new generation, settings generation maps to
profile revision plus one, disabled policy maps to no channels/leads, and a failed
archive sync leaves settings durable with `Pending`.

```rust
state.update_reminder_policy(&permit, update, || irreversible += 1)?;
let first = SettingsStore::new(root.reliable_state())?.load()?.generation();
state.update_reminder_policy(&permit2, update, || {})?;
assert_eq!(SettingsStore::new(root.reliable_state())?.load()?.generation(), first);
```

- [ ] **Step 2: Run RED**

Run:
`cargo +1.97.0 test -p tokenmaster-app state_tests::reminder --lib --locked`

Expected: compile failure for missing update/synchronizer APIs.

- [ ] **Step 3: Implement settings-first desired state**

Add an atomic three-state code. Validate through `tokenmaster_state::ReminderPolicy`.
Skip `SettingsStore::save` only when a persisted current value is identical. Otherwise
begin irreversible, invoke the observer once, save, and reread.

Map settings into the domain profile exactly:

```rust
let revision = load.generation().unwrap_or(0)
    .checked_add(1)
    .filter(|value| *value <= i64::MAX as u64)
    .ok_or_else(ApplicationError::generation_overflow)?;
let channels = load.value().portable().reminders().enabled()
    .then_some(NotificationChannel::InApp)
    .into_iter()
    .collect();
```

Set `Synchronized` only after the store commit; set `Pending` on every error.

- [ ] **Step 4: Project exact state and run GREEN**

Add `DesktopReminderPolicy` to `reliable_state_projection_for_outcome` from the same
loaded settings snapshot and atomic state.

Run:
`cargo +1.97.0 test -p tokenmaster-app state_tests --lib --locked`

Expected: all app state tests pass.

- [ ] **Step 5: Commit**

```powershell
git add -- crates/app/src/state.rs crates/app/src/state_tests.rs
git diff --cached --check
git commit -m "feat(app): persist reminder policy"
```

### Task 4: Bind every application lifecycle path

**Files:**
- Modify: `crates/app/src/command.rs`
- Modify: `crates/app/src/application.rs`
- Modify: `crates/app/src/application_tests.rs`
- Modify: `crates/app/src/command_tests.rs` if command admission coverage lives there

**Interfaces:**
- Adds `ApplicationCommand::UpdateReminderPolicy`.
- Adds bounded `ApplicationOperationPayload::ReminderPolicy(ApplicationReminderPolicyUpdate)`.
- Reuses `ApplicationStateOwner::synchronize_reminder_profile` for startup, Save,
  config-import confirmation, and restored-bundle startup.

- [ ] **Step 1: Write RED command/lifecycle tests**

Prove Desktop intent maps to one redacted bounded payload, startup sync occurs before
reminder runtime construction, Save uses the existing worker, config import triggers
the same sync, store failure leaves the rest of the live bundle available, and one
post-start reliable projection replaces initial `pending`.

- [ ] **Step 2: Run RED**

Run:
`cargo +1.97.0 test -p tokenmaster-app reminder_policy --lib --locked`

Expected: compile/assertion failure because no command/lifecycle binding exists.

- [ ] **Step 3: Implement command execution**

Map the command to `DesktopOperationKind::UpdatePolicy`, execute settings persistence,
then call synchronization. Archive sync failure keeps the command durable-success path
but atomic sync state remains `Pending`. On success:

```rust
if let Some(reminder) = bundle.reminder.owner() {
    let _ = reminder.lock().map_err(|_| ApplicationError::internal())?
        .notify_profile_changed();
}
bundle.controller.refresh(DesktopRefreshUrgency::Hint)
    .map_err(|_| ApplicationError::controller())?;
```

At startup, a sync error constructs `OptionalReminderRuntime::failed(StoreUnavailable)`
and continues the other owners. After bundle startup publish the refreshed reliable
projection once. Config import and restored startup use the same synchronizer.

- [ ] **Step 4: Run GREEN**

Run:
`cargo +1.97.0 test -p tokenmaster-app --locked`

Expected: all app unit/integration/adversarial tests pass.

- [ ] **Step 5: Commit**

```powershell
git add -- crates/app/src/command.rs crates/app/src/application.rs crates/app/src/application_tests.rs crates/app/src/command_tests.rs
git diff --cached --check
git commit -m "feat(app): apply reminder settings live"
```

### Task 5: Responsive bounded Settings editor

**Files:**
- Modify: `crates/desktop/ui/main.slint`
- Modify: `crates/desktop/ui/views/settings-view.slint`
- Modify: `crates/desktop/src/ui.rs`
- Modify: `crates/desktop/tests/recovery_ui_contract.rs`
- Create: `crates/desktop/tests/reminder_settings_ui_contract.rs`

**Interfaces:**
- Adds Slint `ReminderCustomLeadRow { enabled, value, unit-index }`.
- Adds exactly eight-row `reminder-custom-lead-rows`.
- Adds callbacks for row edit, typed Save, recommended reset, and dirty state.

- [ ] **Step 1: Write RED real-UI tests**

Create a headless software-renderer contract. Apply synchronized `[604800, 10800, 90]`,
assert 7d preset checked, custom rows display `3 hours` and `90 seconds`, edit a row,
submit, and capture one exact `DesktopIntent::UpdateReminderPolicy`. Cover duplicate,
overflow, disabled, coordinator rejection, recommended reset, dirty-draft preservation,
wide/narrow layout, and accessible labels.

- [ ] **Step 2: Run RED**

Run:
`cargo +1.97.0 test -p tokenmaster-desktop --test reminder_settings_ui_contract --locked`

Expected: compile failure because the generated Slint properties/callbacks are absent.

- [ ] **Step 3: Add fixed editor UI**

Import `ScrollView` and `ComboBox`; add the reminder card before backup policy. Use
five preset checkboxes and this bounded repeated editor shape:

```slint
for row[index] in root.custom-lead-rows: HorizontalLayout {
    CheckBox { checked: row.enabled; toggled => { root.custom-lead-edited(index, self.checked, value.value, unit.current-index); } }
    value := SpinBox { minimum: 1; maximum: root.maximum-for-unit(unit.current-index); value: row.value; edited(v) => { root.custom-lead-edited(index, enabled.checked, v, unit.current-index); } }
    unit := ComboBox { model: ["seconds", "minutes", "hours", "days"]; current-index: row.unit-index; selected(_) => { root.custom-lead-edited(index, enabled.checked, value.value, self.current-index); } }
}
```

Keep one local static feedback string and one dirty bit; no line edit, timer, or dynamic
row addition.

- [ ] **Step 4: Implement exact Rust mapping**

Use largest exact unit in `apply_reliable_state_projection`. Row callbacks call
`ModelRc::set_row_data`. Save collects enabled rows, uses checked multiplication,
combines preset values, calls `DesktopIntent::update_reminder_policy`, and clears dirty
only for non-rejected admission. Stable messages contain no values or paths.

- [ ] **Step 5: Run GREEN and package tests**

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-desktop --test reminder_settings_ui_contract --locked
cargo +1.97.0 test -p tokenmaster-desktop --test recovery_ui_contract --locked
cargo +1.97.0 test -p tokenmaster-desktop --locked
```

Expected: every test passes and the UI remains responsive at narrow/wide widths.

- [ ] **Step 6: Commit**

```powershell
git add -- crates/desktop/ui/main.slint crates/desktop/ui/views/settings-view.slint crates/desktop/src/ui.rs crates/desktop/tests/recovery_ui_contract.rs crates/desktop/tests/reminder_settings_ui_contract.rs
git diff --cached --check
git commit -m "feat(desktop): edit reminder settings"
```

### Task 6: Computed audits, contracts, review, and developer closure

**Files:**
- Modify: `scripts/audit-application-composition.ps1`
- Modify: `scripts/tests/audit-application-composition.Tests.ps1`
- Modify: `scripts/audit-desktop-shell.ps1`
- Modify: `scripts/tests/audit-desktop-shell.Tests.ps1`
- Modify: `scripts/audit-benefit-inventory.ps1`
- Modify: `spec/SPECIFICATION.md`
- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/API_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `README.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/AUDIT_AND_MASTER_PLAN.md`
- Modify: `docs/FEATURE_PARITY.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/CHANGELOG.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: this plan

**Interfaces:** Computed source/release receipts and mutation tests become executable
guards for the accepted design; normative/operational documents remain synchronized.

- [ ] **Step 1: Add RED audit mutations**

Mutate away generation binding, settings-first order, global-only SQL, override
preservation, scope/lot/lead caps, sync-state publication, startup/import binding,
single operation worker, eight-row cap, dirty-draft preservation, checked conversion,
accessibility, and no-timer/no-polling rules. Each mutation must fail with one stable
`TM-*` code.

- [ ] **Step 2: Run RED then implement computed receipts**

Run:
`Invoke-Pester -Path scripts/tests/audit-application-composition.Tests.ps1,scripts/tests/audit-desktop-shell.Tests.ps1 -Output Detailed`

Expected RED: new mutations fail. Implement computed counts/order checks, rerun, and
require all mutation tests pass without retaining `testResults.xml`.

- [ ] **Step 3: Synchronize documentation**

Mark only global reminder settings synchronization/editing implemented. Keep per-scope
editing, snooze, quiet hours, OS/tray delivery, usage alerts, activation, P4/P5/P6,
M0, package/signing/soak/release explicitly incomplete. Add the next ADR and no commit
hash to tracked files.

- [ ] **Step 4: Focused verification and independent review**

```powershell
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy -p tokenmaster-store -p tokenmaster-state -p tokenmaster-desktop -p tokenmaster-app --all-targets --locked
cargo +1.97.0 test -p tokenmaster-store --test benefit_write_contract --locked
cargo +1.97.0 test -p tokenmaster-app --locked
cargo +1.97.0 test -p tokenmaster-desktop --locked
pwsh -NoProfile -File scripts/audit-benefit-inventory.ps1 -RepositoryRoot (Get-Location).Path
pwsh -NoProfile -File scripts/audit-application-composition.ps1 -RepositoryRoot (Get-Location).Path
pwsh -NoProfile -File scripts/audit-desktop-shell.ps1 -RepositoryRoot (Get-Location).Path
```

Dispatch one independent read-only Sol High review for cross-file crash consistency,
revision concurrency, overrides/deliveries, startup/import/restore, lock order, UI
bounds/accessibility/privacy, and audit sufficiency. Add RED tests for every Critical or
Important finding and repeat until both counts are zero.

- [ ] **Step 5: Exact baseline**

```powershell
pwsh -NoProfile -File scripts/audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

Expected: every command exits zero. This is developer closure only.

- [ ] **Step 6: Commit synchronized evidence and prove clean state**

Stage explicit audit/spec/doc paths, run `git diff --cached --check`, commit
`docs: close reminder settings synchronization`, then require `git status --short` empty
and `testResults.xml` absent. Do not push or claim release acceptance.
