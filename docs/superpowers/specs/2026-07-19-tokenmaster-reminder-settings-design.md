# TokenMaster Reminder Settings Synchronization Design

**Status:** Approved for autonomous implementation by the existing TokenMaster product
direction and the operator's explicit `go` / self-select-the-optimal-option instruction.

**Scope:** One portable global in-app reminder profile, its exact projection into the
benefit archive, and a bounded responsive Settings editor. Per-scope overrides, snooze,
quiet hours, OS/tray delivery, usage alerts, activation, skins, localization, CLI/MCP,
M0, packaging, signing, and release remain separate slices.

## 1. Outcome

The user can enable or disable expiry reminders, select any subset of the recommended
7d/24h/12h/6h/1h leads, replace or extend them with up to eight total custom leads,
reset explicitly to the recommended profile, save without blocking Slint, and see
whether the durable settings have reached the live reminder archive.

The design preserves these existing invariants:

- Rust 1.97, Slint 1.17, bundled SQLite, one root Cargo workspace;
- reliable settings are the only portable desired-state authority;
- the archive remains the effective scheduling/query projection;
- one through eight unique enabled leads, each 60 through 31,536,000 seconds;
- a disabled profile has zero leads and zero notification channels;
- settings generation and global reminder-profile revision are positive and monotonic;
- scope overrides remain exact and are never rewritten by a global edit;
- no UI timer, polling loop, worker, raw path, identity, SQL, or runtime authority;
- one application operation worker remains the only settings mutation executor;
- retained UI state is fixed at five recommended toggles plus eight custom editor rows.

## 2. Existing truth and gap

`tokenmaster-state` schema 1 already stores one portable `ReminderPolicy`. The benefit
archive independently seeds one global `ReminderProfile` and supports scope overrides.
The Notifications route reads the archive profile, while Settings currently projects
and edits only backup policy. Config import/restore can therefore change portable
reminder settings without changing the live scheduling profile.

The missing contour is a generation-bound desired-state projection plus a typed UI
intent. It is not a new provider setting, a route-time query, or a new persistence
format.

## 3. Alternatives

### A. Settings-authoritative generation-bound projection — selected

Persist the bounded portable policy first. Map settings generation `N` to global
profile revision `N + 1` (`None` defaults map to revision 1), then atomically replace
only the SQLite global profile and rebuild inherited due rows. Startup, explicit save,
config import, and restored-bundle startup use the same synchronizer. A failed archive
projection leaves the durable desired state intact and exposes `pending`; repeating Save
or restarting retries the same generation without creating another settings record.

This gives one portable authority, exact crash recovery, idempotence, visible partial
state, and no cross-file transaction claim.

### B. Archive-authoritative editing — rejected

Editing SQLite first and later copying into reliable settings makes backup/import state
secondary, creates two-way conflict resolution, and can export values different from
the live UI.

### C. Runtime-only overlay — rejected

Keeping custom leads only in the reminder runtime would diverge Notifications queries,
lose state on restart, bypass config packages, and require an additional retained
runtime policy owner.

## 4. Store transaction

Add
`UsageStore::set_benefit_reminder_global_profile(&ReminderProfile) -> Result<BenefitProfileApplyResult, StoreError>`.

Inside one `IMMEDIATE` transaction it:

1. reads and validates the existing global profile and global benefit state;
2. returns an exact no-op when revision and value are identical;
3. rejects a lower revision as `StaleRevision` and same-revision equivocation as
   `InvalidValue`;
4. selects at most 32 inheriting scopes with one-row lookahead and at most 256 total
   current lots; capacity excess fails before mutation;
5. replaces only the global row/thresholds and never deletes scope overrides;
6. rebuilds due rows only for inheriting scopes using their exact current lots;
7. updates the global pending count and increments benefit revision when a published
   benefit dataset exists; revision zero remains zero before first publication;
8. commits all-or-nothing and returns the resulting revision and total pending count.

Delivered/outbox/acknowledgement rows are not deleted. Re-adding a previously delivered
lead therefore remains suppressed by the existing delivery identity.

## 5. Application synchronization

`ApplicationStateOwner` owns a constant-size atomic sync state:

- `pending`: durable settings are not yet proven equal to the archive projection;
- `synchronized`: the exact settings-derived revision/value committed to SQLite;
- `unavailable`: no usable live archive can currently receive the projection.

The synchronizer loads validated settings, computes revision `generation + 1` with
checked `i64` range, maps enabled policy to the in-app channel and disabled policy to
no channels/leads, calls the store transaction, and changes the atomic state only after
the transaction result. Errors remain path-private.

Startup attempts synchronization after archive migration/recovery is complete and
before starting `BenefitReminderRuntime`. Failure disables only the optional reminder
runtime; usage, quota, Git, Desktop, recovery, and backup owners remain available.
After startup the reliable-state projection is republished once so the initial
`pending` view cannot remain stale.

Explicit Save persists settings first. If the requested policy already equals a
persisted value, it reuses the current settings generation so retrying a pending sync
does not grow records. Archive synchronization then runs on the existing application
operation worker. A projection failure does not roll back or misreport the durable
settings commit: the operation succeeds as a settings update and the UI reports
`pending`. A successful projection sends one reminder profile-change hint and one
controller refresh; neither waits on the UI thread.

Config-import confirmation runs the same synchronizer after its portable settings
commit. Data-plus-portable-settings restore reaches the same path while starting the
replacement bundle. Data-only and automatic recovery never change reminder settings.

## 6. Desktop projection and intent

Add a copyable `DesktopReminderPolicy` to `DesktopReliableStateSummary`:

- `enabled: bool`;
- fixed `[u32; 8]` storage plus `lead_count: u8`;
- `DesktopReminderSyncState`;
- validated descending unique values only.

Add `DesktopIntent::UpdateReminderPolicy { enabled, lead_seconds: Box<[u32]> }` and a
constructor that rejects more than eight values, duplicates, invalid bounds, and the
invalid enabled/empty combinations. `Debug` prints only a redacted marker.

The app maps that intent to `ApplicationCommand::UpdateReminderPolicy` and one bounded
payload. The existing capacity-one operation coordinator provides started/queued/
coalesced/rejected admission and publishes running/atomic/terminal state normally.

## 7. Settings UI

The existing Settings route becomes scrollable and gains one reminder card before the
backup card. It contains:

- an enable checkbox;
- five independently selectable recommended leads: 7 days, 24 hours, 12 hours,
  6 hours, 1 hour;
- exactly eight fixed custom rows, each with enabled checkbox, bounded integer SpinBox,
  and seconds/minutes/hours/days unit ComboBox;
- `Save reminder profile` and `Save recommended profile` actions;
- concise constraints, stable submission feedback, and visible sync state.

Exact imported values are lossless: each custom value uses the largest exact unit;
non-divisible values remain seconds. Recommended values appear only as checkboxes, not
duplicated custom rows. Conversion is checked in Rust; total values are deduplicated,
sorted descending, and rejected rather than silently truncated.

One eight-row `VecModel` is replaced from reliable projection and edited in place by
row callbacks. A fixed dirty bit prevents unrelated reliable-state publications from
overwriting an unfinished draft. Accepted submission clears the dirty bit; rejected
submission retains the draft. There is no text parser, unbounded input, timer,
animation, polling loop, or per-row worker.

Wide layout may place custom rows in two visual columns; narrow layout uses one column.
All checkboxes, value controls, units, actions, feedback, and sync status have concise
accessible labels. P4 still owns final en/ru/pseudo localization and skin switching.

## 8. Failure semantics

- Invalid UI input: no intent and a stable inline message.
- Busy operation coordinator: draft retained and stable busy feedback.
- Settings save failure: operation fails; archive and live runtime are untouched.
- Archive Busy/unavailable after settings commit: settings operation succeeds, sync
  state is `pending`, existing effective archive profile remains atomic, Save/restart
  retries the same generation.
- Same revision with different archive value: fail closed as equivocation and show
  `pending`.
- Reminder runtime unavailable after successful archive commit: sync remains
  `synchronized`; runtime health independently remains unavailable until restart.
- Import/restore/startup: no partial SQLite profile or due rebuild is visible.
- Shutdown: no new worker exists and existing join order is unchanged.

## 9. Performance and memory

The store mutation is bounded to 32 scopes, 256 lots, eight leads each, and one SQLite
transaction. The UI retains five booleans and eight small rows. The command payload
retains at most eight `u32` values. No history, provider payload, identity, path, SQL,
or settings JSON crosses into Slint. Route selection performs no settings load or
mutation.

## 10. Verification

RED/GREEN coverage must prove:

- global replacement, no-op, stale/equivocation rejection, override preservation,
  disabled profile, delivered-key survival, and transaction rollback;
- defaults/custom/subset/import/restart settings generations map exactly to profile
  revision and retry does not create an extra generation;
- startup failure isolates only reminder runtime and publishes pending state;
- typed Desktop validation, fixed eight-row draft, exact unit round-trip, duplicate/
  overflow rejection, dirty-draft preservation, responsive UI, and accessibility;
- one operation worker, no UI/runtime authority expansion, no unbounded model/input,
  and mutation audits that fail when each invariant is removed;
- focused store/state/app/Desktop suites, strict Clippy, source/release audits,
  independent Critical/Important review, and the exact repository baseline.

This is developer closure only. It does not claim snooze, quiet hours, OS/tray
delivery, usage alerts, activation, P4/P5/P6, M0, packaging, signing, soak, or release.
