# TokenMaster Bounded Rhythm Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add exact bounded 24-hour and seven-weekday rhythm distributions to the Activity route through the existing recent analytics envelope.

**Status:** Completed on 2026-07-22. The store behavioral coverage landed in the
private analytics unit plus `rhythm_contract` rather than a new integration target;
existing analytics cancellation/generation tests remain the shared transaction proof.

**Architecture:** Query privately converts the selected local 1/7/30-day range into tagged UTC minute/hour rollup segments. Store aggregates those segments inside the existing analytics transaction; product/controller reuse the current History envelope and worker; Desktop adds an independent rhythm subprojection beside Recent Activity.

**Tech Stack:** Rust 1.97, bundled SQLite, Jiff 0.2.32, Slint 1.17.

## Global Constraints

- Keep the root Cargo workspace and existing capacity-one Desktop query worker.
- Retain at most 24 hourly and seven weekday buckets; request range is at most 30 civil days.
- Retain at most 768 resolved occurrences and 2,304 UTC rollup segments.
- Never query raw history for rhythm and never expose paths, identities, content, cursors, or authority.
- Recent Activity and rhythm keep independent evidence states.
- Use focused RED/GREEN tests, one implementation review, one final re-review, then one baseline gate.

---

### Task 1: Exact query and store rhythm contract

**Files:**
- Modify: `crates/query/src/calendar.rs`
- Modify: `crates/query/src/analytics.rs`
- Modify: `crates/query/src/lib.rs`
- Modify: `crates/store/src/usage/query/analytics.rs`
- Modify: `crates/store/src/usage/query.rs`
- Modify: `crates/store/src/usage/mod.rs`
- Modify: `crates/store/src/lib.rs`
- Test: `crates/query/tests/rhythm_contract.rs`
- Test: `crates/store/tests/aggregate_query_contract.rs`

**Interfaces:**
- Produces: `UsageRhythmSelection::{None, HourAndWeekday}` and `UsageAnalyticsRequest::with_rhythm`.
- Produces: `UsageRhythm`, `UsageRhythmHour`, `UsageRhythmWeekday`, and `UsageWeekday`.
- Produces privately: at most 768 local occurrences and 2,304 tagged store segments.
- Produces in store: optional `UsageRhythmQuery` and canonical 24+7 capture rows inside `UsageAnalyticsCapture`.

- [x] **Step 1: Write failing query value/calendar tests**

Add tests that request rhythm for 1/7/30 days, reject 31 days, assert 24+7 canonical rows,
and cover UTC, New York gap/fold, Lord Howe, Kathmandu, and Apia exposure semantics.

- [x] **Step 2: Verify RED**

Run: `cargo +1.97.0 test -p tokenmaster-query --test rhythm_contract --locked`

Expected: compilation fails because the rhythm request/value API does not exist.

- [x] **Step 3: Implement the minimal private timezone plan and public values**

Add the fixed selection and builder without changing existing constructor call sites.
Walk UTC minutes into offset-qualified local-hour occurrences, compose each occurrence
into aligned store segments, and retain only checked occurrence/segment counters plus
the final 24+7 public output.

- [x] **Step 4: Write and verify the failing store aggregate test**

The store test must assert canonical 31 rows, exact overview/hour/weekday equality,
scope filtering, generation fencing, deadline cleanup, and SQL text/plan containing
`usage_time_rollup` but neither raw event table nor `OFFSET`.

Run: `cargo +1.97.0 test -p tokenmaster-store --test aggregate_query_contract rhythm --locked`

Expected: compilation fails because the typed store rhythm query/capture is absent.

- [x] **Step 5: Implement one-transaction store aggregation**

Add the bounded tagged-segment validator and internal `VALUES` CTE. Return all canonical
rows, synthesize only structurally empty metrics for absent groups, reject duplicate or
out-of-range rows, and compare both distribution sums with the captured overview.

- [x] **Step 6: Verify GREEN**

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-store --test aggregate_query_contract rhythm --locked
cargo +1.97.0 test -p tokenmaster-query --test rhythm_contract --locked
cargo +1.97.0 test -p tokenmaster-query --test recent_history_contract --locked
```

Expected: all selected tests pass with no warnings.

### Task 2: Reuse the existing History worker and product envelope

**Files:**
- Modify: `crates/desktop/src/controller.rs`
- Modify: `crates/product/src/snapshot.rs` only if required by type propagation
- Test: `crates/desktop/tests/controller_contract.rs`
- Test: `crates/desktop/tests/history_range_controller_contract.rs`

**Interfaces:**
- Consumes: `UsageAnalyticsRequest::with_rhythm(HourAndWeekday)`.
- Produces: every default and interactive History request includes rhythm; Dashboard remains `None`.

- [x] **Step 1: Write failing controller tests**

Record both analytics requests. Assert Dashboard uses `None`; default 30-day and
interactive 1/7/30 recent requests use `HourAndWeekday`; no extra source call occurs.
Assert stale/failed range work publishes neither a daily/rhythm mix nor a new snapshot.

- [x] **Step 2: Verify RED**

Run: `cargo +1.97.0 test -p tokenmaster-desktop --test history_range_controller_contract rhythm --locked`

Expected: the recent request reports `None`.

- [x] **Step 3: Implement minimal request wiring**

Enable rhythm only in `DesktopQueryPlan::history_request`; keep today Dashboard,
activity page, controller worker, reducer, generations, and terminal rollback unchanged.

- [x] **Step 4: Verify GREEN**

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-desktop --test controller_contract --locked
cargo +1.97.0 test -p tokenmaster-desktop --test history_range_controller_contract --locked
```

Expected: both targets pass and recorded query count is unchanged.

### Task 3: Add bounded Activity rhythm projection and Slint view

**Files:**
- Modify: `crates/desktop/src/activity.rs`
- Modify: `crates/desktop/src/ui.rs`
- Modify: `crates/desktop/ui/models.slint`
- Modify: `crates/desktop/ui/main.slint`
- Modify: `crates/desktop/ui/views/activity-view.slint`
- Test: `crates/desktop/tests/activity_projection_contract.rs`
- Test: `crates/desktop/tests/ui_contract.rs`

**Interfaces:**
- Consumes: `snapshot.activity()` for newest rows and `snapshot.history()` for rhythm.
- Produces: independent rhythm state/reasons/range/timezone and exactly 24 hourly plus seven weekday rows.

- [x] **Step 1: Write failing projection tests**

Assert exact 24+7 order, exposure/occurrence/event/token mapping, independent retained/
unavailable states, range replacement, 10,000 snapshot replacements, and privacy-safe
Debug. Assert Recent Activity remains ready when rhythm is unavailable.

- [x] **Step 2: Verify RED**

Run: `cargo +1.97.0 test -p tokenmaster-desktop --test activity_projection_contract rhythm --locked`

Expected: compilation fails because the rhythm projection API is absent.

- [x] **Step 3: Implement projection and UI model application**

Map 24+7 fixed rows from the History envelope, keep a separate section state, and
replace two bounded Slint models once per accepted product generation. Format exposure,
occurrences, events, and token availability without presenting unavailable as zero.

- [x] **Step 4: Write failing compiled UI tests**

Assert wide/narrow rendering, 24+7 model lengths/order, accessible timezone/range/DST
meaning, visible skipped versus exposed-empty state, route-only navigation, and the
unchanged newest-event table.

- [x] **Step 5: Verify RED then GREEN**

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-desktop --test ui_contract rhythm --locked
cargo +1.97.0 test -p tokenmaster-desktop --test activity_projection_contract --locked
cargo +1.97.0 test -p tokenmaster-desktop --test ui_contract --locked
```

Expected: the new named UI test first fails before Slint wiring, then all three targets pass.

### Task 4: Contract synchronization and bounded acceptance

**Files:**
- Modify: `spec/SPECIFICATION.md`
- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/API_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `docs/FEATURE_PARITY.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: `CHANGELOG.md`
- Modify: relevant Desktop/source audit scripts and tests only for the new fixed models/contract

**Interfaces:**
- Consumes: verified implementation and focused test receipts.
- Produces: a truthful P3-D rhythm closure without P4/P5/P6/M0/release claims.

- [x] **Step 1: Run one implementation review**

Review only the changed contract/code for correctness, DST, privacy, bounded memory,
and no-new-owner invariants. Fix only product/security/required-evidence findings with
focused RED/GREEN tests.

- [x] **Step 2: Update durable contracts and audit allowlists**

Record exact 30-day, 768-occurrence, 2,304-segment, 24+7, exposure, independent-state,
and no-raw-history boundaries. Mark the parity row implemented only after tests pass.

- [x] **Step 3: Run focused aggregate gates**

```powershell
cargo +1.97.0 test -p tokenmaster-query --test rhythm_contract --locked
cargo +1.97.0 test -p tokenmaster-desktop --test activity_projection_contract --locked
cargo +1.97.0 test -p tokenmaster-desktop --test ui_contract --locked
pwsh -NoProfile -File scripts\audit-desktop-shell.ps1 -RepositoryRoot (Get-Location).Path
pwsh -NoProfile -File scripts\audit-application-composition.ps1 -RepositoryRoot (Get-Location).Path
```

- [x] **Step 4: Run one final re-review and the baseline once**

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy --workspace --all-targets --locked
$env:CARGO_BUILD_JOBS = '1'
cargo +1.97.0 test --workspace --locked
```

Expected: all gates exit zero. Do not repeat them unless source or validator inputs change.

- [x] **Step 5: Commit and cleanup**

Stage only task-owned files, commit with an English conventional message, verify clean
Git state, stop task-owned agents/processes, and remove only proven disposable heavy
artifacts after final evidence is recorded.
