# P1-E Immutable Publication and Suspend/Resume Plan

Status: approved executable plan.

## Goal

Complete the P1 runtime boundary with one small immutable `EngineSnapshot` that is
available immediately from current archive truth, advances monotonically only for a
newer archive publication, cannot be replaced by an older/failed result, survives
burst/pause/resume/restart behavior truthfully, and retains no archive-sized or private
state. Then bind the existing logical pause/resume contract to Windows power events and
close P1 with race and resource evidence.

## Architecture

`UsageStore` remains the archive authority. Runtime reads its fixed
`ArchivePublication` plus the completion time of the referenced exact scan set and
converts them into provider-neutral scalar values. One fixed
`Arc<Mutex<EnginePublicationState>>` owns exactly one `Copy` snapshot and fixed
checked counters. It never owns a SQLite connection, event page, source, path,
checkpoint, watcher, UI handle, or prior snapshot list.

The in-process snapshot generation starts at one from startup archive truth. A
candidate replaces it only when `archive_generation` is strictly newer. Replacement
increments the in-process generation with checked arithmetic. Equal candidates are
coalesced; older candidates are rejected and counted. Failed, busy, cancelled, or
deadline outcomes can update fixed operational counters but cannot create a newer
archive snapshot. On the next real publication, its immutable diagnostic copy includes
those counters. Generation restarts per process; persisted archive generation remains
the cross-process/restart order.

`data_through_ms` is the completion timestamp of the exact
`latest_complete_scan_set`, not wall-clock callback time and not the maximum event
timestamp. Missing scan provenance remains `None`.

## Bounds and invariants

- exactly one retained engine snapshot and one fixed diagnostics struct;
- all generation/counter increments are checked; overflow sets a fail-closed flag and
  never wraps or replaces newer truth;
- archive revision, scan-set ID, and archive generation are scalar copies only;
- quality is `empty`, `complete`, `partial`, or `recovery_pending`; no fabricated
  freshness or zero;
- consumers replace local presentation state only when
  `candidate.is_newer_than(current)`;
- snapshot publication happens after archive mutation commits and after the writer
  guard is released;
- publication failure changes the refresh result to failed/unavailable but never rolls
  back or hides the already committed archive; the next reconciliation retries from
  archive truth;
- suspend closes admission before cancellation; resume invalidates watcher assumptions
  and forces authoritative reconciliation; power callbacks never touch SQLite;
- no M0, packaging, signing, interactive, or release claim follows from P1-E.

## Task 1 — Add the archive data-through lookup

Files:

- modify `crates/store/src/usage/scan.rs`;
- add/modify focused store tests beside the scan contracts.

TDD:

1. Add a failing test that retrieves a completed scan set by ID and proves exact
   completion time/outcome, stale-ID rejection, and no page/source allocation.
2. Run the exact test and capture RED.
3. Add `UsageStore::scan_set_snapshot(ScanSetId)` as a thin checked wrapper over the
   existing indexed singleton lookup.
4. Run the focused scan/store targets and strict Clippy.

## Task 2 — Define immutable provider-neutral snapshot values

Files:

- add `crates/runtime/src/publication.rs`;
- modify `crates/runtime/src/lib.rs`;
- add `crates/runtime/tests/publication_contract.rs`.

Values:

- `EngineSnapshotGeneration`;
- `EnginePublicationQuality`;
- `EngineDiagnostics` with completed/busy/cancelled/deadline/failed/equal/older and
  overflow facts;
- `EngineSnapshot` with in-process generation, archive generation, optional archive
  revision, optional exact scan set, optional data-through time, quality, and
  diagnostics.

TDD:

1. Write RED contracts for scalar getters, `Copy`/Debug privacy, strict newer
   comparison, equal/older rejection, checked generation/counter overflow, and one-
   snapshot retention across 10,000 candidates.
2. Implement the smallest fixed-state publisher.
3. Run `cargo +1.97.0 test -p tokenmaster-runtime --test publication_contract --locked`.

## Task 3 — Integrate startup seeding and post-refresh publication

Files:

- modify `crates/runtime/src/live.rs`;
- modify `crates/runtime/src/lifecycle.rs`;
- modify `crates/runtime/tests/live_runtime_contract.rs` or the publication contract.

TDD:

1. Add RED integration coverage proving startup exposes current archive truth without
   waiting for a scan; an append produces a strictly newer in-process and archive
   generation; revision/scan/quality/data-through match SQLite; and an older consumer
   snapshot cannot replace the newer one.
2. Add RED writer-contention coverage proving `busy` does not advance the engine
   generation; after contention clears, a completed reconciliation advances it and
   carries the busy counter.
3. Add one shared publication state to `LiveRuntime` and `LiveExecution`; seed before
   worker start, publish after every observed newer archive generation, and include the
   immutable engine snapshot in `LiveRuntimeSnapshot`.
4. Make a store/publication read or lock failure return fixed failed/unavailable state
   without paths or wrapped errors.
5. Run focused runtime tests and privacy Debug assertions.

## Task 4 — Close race, burst, pause/resume, and restart semantics

Files:

- modify `crates/runtime/tests/live_runtime_contract.rs`;
- modify runtime publication/lifecycle code only when a failing contract proves it.

TDD matrix:

- 10,000 hints still cause at most one aggregate follow-up and bounded snapshot state;
- repeated no-change scans may advance archive freshness but never regress revision,
  scan-set ID, quality, or data-through time;
- pause closes admissions and a cancelled request cannot publish over a newer result;
- resume forces one exact reconciliation and advances only from archive truth;
- replacement/truncation may publish newer `recovery_pending`, but prior canonical
  truth remains readable until rebuild completes;
- process restart resets only the in-process generation and seeds the persisted archive
  generation/revision/scan/data-through exactly;
- consumer races always keep the highest snapshot generation.

## Task 5 — Bind Windows power events without UI/archive coupling

Files:

- add a small Windows-only power-event adapter in `crates/platform` or the future
  desktop shell boundary;
- modify runtime lifecycle integration through a narrow `pause`/`resume` command;
- add Windows-focused contracts.

The callback reduces suspend/resume notifications to a capacity-one lifecycle signal.
It must not acquire the writer lease, open SQLite, retain a window handle in engine
state, or call runtime while holding the OS callback lock. Duplicate suspend/resume,
resume-before-suspend, shutdown races, hibernation, and clock rollback are idempotent.
If a reliable power notification cannot be installed, the periodic scheduler remains
the recovery backstop and the UI reports the integration unavailable.

## Task 6 — P1-E evidence and project truth

Files:

- update `spec/TRACEABILITY.md`, affected security/data/decision documents,
  `docs/CURRENT_STATE.md`, `docs/HANDOFF.md`, `docs/ROADMAP.md`,
  `docs/CHANGELOG.md`, and `docs/PROJECT_HISTORY.md`;
- add an ADR only if implementation changes this approved architecture.

Focused evidence:

- generation race and old-result rejection;
- busy/cancel/deadline/partial/recovery behavior;
- burst and restart;
- Windows suspend/resume/hibernate sequence;
- repeated runtime generation resource return;
- bounded private-memory slope, CPU idle/refresh budget, handles, threads, USER and GDI
  objects using the existing evidence methodology.

Final gate:

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
git diff --check
```

The one-million-row test may remain explicitly ignored in the normal workspace run;
its dedicated release-scale invocation remains a separate gate. P1-E completion does
not accept M0 without the exact interactive and uninterrupted-soak receipts.
