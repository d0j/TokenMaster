# TokenMaster P3-D.1 History Route Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the production History placeholder with a truthful bounded 30-day usage-history route.

**Architecture:** Add one `recent_days(30)` analytics request to the existing capacity-one desktop refresh, publish it as an independent product section, map it into an owned 30-row desktop projection, and replace only the History placeholder with a compiled Slint view. Dashboard remains today-only and no new worker, timer, cache, or database authority is introduced.

**Tech Stack:** Rust 1.97.0, Slint 1.17, bundled SQLite, existing `tokenmaster-query`, `tokenmaster-product`, `tokenmaster-desktop`, PowerShell verification.

## Global Constraints

- TokenMaster is the only product; WhereMyTokens and ccusage remain pinned references, not dependencies.
- Keep input, retained memory, archive writes, query results, and UI models bounded.
- Never expose prompts, responses, reasoning, commands, source contents, credentials, raw incomplete lines, or absolute user paths.
- Production Slint callbacks perform no SQLite, provider, filesystem, process, network, or blocking work.
- The existing single capacity-one refresh worker and one immutable `ProductSnapshot` remain the only live publication path.
- Do not add crates, dependencies, schema changes, backup/recovery features, CLI/MCP authority, or arbitrary SQL/shell/HTTP/filesystem access.
- Every production behavior change follows red-green-refactor and every task ends with focused verification.

---

### Task 1: Add an exact bounded recent-days query range

**Files:**
- Modify: `crates/query/src/analytics.rs`
- Modify: `crates/query/src/calendar.rs`
- Modify: `crates/query/tests/analytics_value_contract.rs`
- Create: `crates/query/tests/recent_history_contract.rs`

**Interfaces:**
- Produces: `UsageRange::recent_days(day_count: u16) -> Result<UsageRange, QueryError>`.
- Produces: stable range code `recent_days` and exact `[today-(N-1), tomorrow)` resolution.
- Consumes: existing `CalendarBoundaryResolver`, `UsageAnalyticsRequest`, and `QueryService`.

- [ ] **Step 1: Write the failing public validation test**

```rust
#[test]
fn recent_days_is_bounded_and_has_a_stable_code() {
    assert_eq!(UsageRange::recent_days(30).expect("30 days").stable_code(), "recent_days");
    assert_eq!(UsageRange::recent_days(0).expect_err("zero").code(), QueryErrorCode::InvalidValue);
    assert_eq!(UsageRange::recent_days(401).expect_err("over cap").code(), QueryErrorCode::CapacityExceeded);
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run: `cargo +1.97.0 test -p tokenmaster-query --test analytics_value_contract recent_days_is_bounded_and_has_a_stable_code --locked`

Expected: compile failure because `UsageRange::recent_days` does not exist.

- [ ] **Step 3: Add the minimal range variant and calendar subtraction**

```rust
enum UsageRangeValue {
    Today,
    RecentDays(u16),
    // existing variants
}

impl UsageRange {
    pub fn recent_days(day_count: u16) -> Result<Self, QueryError> {
        if day_count == 0 {
            return Err(QueryError::new(QueryErrorCode::InvalidValue));
        }
        if usize::from(day_count) > MAX_QUERY_SERIES_POINTS {
            return Err(QueryError::new(QueryErrorCode::CapacityExceeded));
        }
        Ok(Self(UsageRangeValue::RecentDays(day_count)))
    }
}
```

Add a crate-private checked `CalendarDate::days_before` using Jiff date arithmetic. In `build_plan`, resolve the local current date, subtract `day_count - 1`, and call the existing `resolver.custom(start, today.tomorrow()?)` path.

- [ ] **Step 4: Write the failing integration test for exact dates and DST**

Create an empty archive with a fixed query clock and request 30 daily points in `America/New_York`. Assert 30 series rows, exact first/last `CalendarDate`, and a spring-forward bucket whose UTC width is 23 hours. Add a 400-day UTC case and assert exactly 400 points.

- [ ] **Step 5: Run the integration test and verify RED, then implement the resolver path**

Run: `cargo +1.97.0 test -p tokenmaster-query --test recent_history_contract --locked`

Expected before implementation: failure because recent-days resolution is missing. Expected after implementation: all tests pass.

- [ ] **Step 6: Run query focused verification**

Run: `cargo +1.97.0 test -p tokenmaster-query --test analytics_value_contract --test recent_history_contract --locked`

Expected: pass with zero failures.

- [ ] **Step 7: Commit**

```powershell
git add -- crates/query/src/analytics.rs crates/query/src/calendar.rs crates/query/tests/analytics_value_contract.rs crates/query/tests/recent_history_contract.rs
git commit -m "feat(query): add bounded recent history range"
```

### Task 2: Publish History as an independent product section

**Files:**
- Modify: `crates/product/src/snapshot.rs`
- Modify: `crates/product/src/reducer.rs`
- Modify: `crates/product/src/route.rs`
- Modify: `crates/product/tests/reducer_contract.rs`
- Modify: `crates/product/tests/route_contract.rs`
- Modify: `crates/product/tests/resource_contract.rs`

**Interfaces:**
- Produces: `ProductSnapshot::history() -> &ProductSection<QueryEnvelope<UsageAnalytics>>`.
- Produces: `ProductReducer::publish_history` and `ProductReducer::fail_history`.
- Preserves: Dashboard route depends on `analytics`; History route depends on `history`.

- [ ] **Step 1: Write failing reducer and route tests**

```rust
assert_eq!(reducer.snapshot().history().kind(), ProductSectionKind::Waiting);
reducer.publish_analytics(attempt(1), today).expect("dashboard analytics");
assert_eq!(reducer.snapshot().route(ProductRoute::History).state(), ProductRouteState::Unavailable);
reducer.publish_history(attempt(1), recent).expect("history analytics");
assert_eq!(reducer.snapshot().route(ProductRoute::History).state(), ProductRouteState::Ready);
reducer.fail_history(attempt(2), QueryErrorCode::DeadlineExceeded).expect("retain history");
assert!(reducer.snapshot().history().retains_payload());
```

Also assert that a new incompatible data-status identity invalidates both analytics sections independently and an older history attempt is rejected.

- [ ] **Step 2: Run tests and verify RED**

Run: `cargo +1.97.0 test -p tokenmaster-product --test reducer_contract --test route_contract --locked`

Expected: compile failure because the History section API does not exist.

- [ ] **Step 3: Implement the minimal section**

Add `history` beside `analytics` in `ProductSnapshot`, initialize it to waiting, expose the accessor, generate reducer methods with `usage_compatible`, invalidate it in `invalidate_incompatible_sections`, and derive `history_ready` from that section plus usage-runtime health. Do not alter other route semantics.

- [ ] **Step 4: Run focused product tests**

Run: `cargo +1.97.0 test -p tokenmaster-product --locked`

Expected: pass with zero failures.

- [ ] **Step 5: Commit**

```powershell
git add -- crates/product/src/snapshot.rs crates/product/src/reducer.rs crates/product/src/route.rs crates/product/tests/reducer_contract.rs crates/product/tests/route_contract.rs crates/product/tests/resource_contract.rs
git commit -m "feat(product): publish independent history analytics"
```

### Task 3: Query History through the existing controller worker

**Files:**
- Modify: `crates/desktop/src/controller.rs`
- Modify: `crates/desktop/tests/controller_contract.rs`

**Interfaces:**
- `DesktopQueryPlan` adds a private `history: UsageAnalyticsRequest` built with `UsageRange::recent_days(30)`, system timezone, daily series, no scopes, and no breakdowns.
- `execute_attempt` publishes/fails History after Dashboard analytics and before quota work.
- `DesktopQuerySource` remains unchanged; it already exposes `usage_analytics`.

- [ ] **Step 1: Add a failing controller test**

Extend the deterministic fake source to record analytics range stable codes. Submit one refresh and assert the exact call sequence is `today`, then `recent_days`; assert the published snapshot has both sections ready. Add a second test where only the recent-days call fails and verify Dashboard analytics remains ready while History is unavailable.

- [ ] **Step 2: Run and verify RED**

Run: `cargo +1.97.0 test -p tokenmaster-desktop --test controller_contract --locked`

Expected: assertion failure because only one analytics request is made.

- [ ] **Step 3: Add the fixed History request and independent publication**

```rust
let history = UsageAnalyticsRequest::new(
    UsageRange::recent_days(30).map_err(map_query_error)?,
    UsageTimeZone::system(),
    WeekStart::Monday,
    UsageSeriesSelection::Daily,
    Vec::new(),
    Vec::new(),
)
.map_err(map_query_error)?;
```

Execute it through the same source and reducer. Preserve cancellation/deadline checks between calls and publish no snapshot until the full attempt reaches the existing publication boundary.

- [ ] **Step 4: Run focused controller tests**

Run: `cargo +1.97.0 test -p tokenmaster-desktop --test controller_contract --locked`

Expected: pass with zero failures.

- [ ] **Step 5: Commit**

```powershell
git add -- crates/desktop/src/controller.rs crates/desktop/tests/controller_contract.rs
git commit -m "feat(desktop): refresh bounded history analytics"
```

### Task 4: Add the bounded desktop History projection

**Files:**
- Create: `crates/desktop/src/history.rs`
- Modify: `crates/desktop/src/lib.rs`
- Modify: `crates/desktop/src/presentation.rs`
- Modify: `crates/desktop/src/dashboard.rs`
- Create: `crates/desktop/tests/history_projection_contract.rs`
- Modify: `crates/desktop/tests/support/dashboard_fixture.rs`
- Modify: `crates/desktop/tests/support/mod.rs`

**Interfaces:**
- Produces: `DesktopHistoryProjection`, `DesktopHistoryRow`, and `MAX_HISTORY_DAYS = 30`.
- `DesktopProjection::history()` exposes the immutable projection.
- Reuses crate-private token/cost/freshness/quality mapping helpers from `dashboard.rs`.

- [ ] **Step 1: Write failing projection tests**

Cover:

```rust
assert_eq!(initial.history().rows().len(), 0);
assert_eq!(initial.history().tokens().availability(), DesktopValueAvailability::Unavailable);
assert_eq!(ready.history().rows().len(), 30);
assert!(ready.history().rows().windows(2).all(|pair| pair[0].date() > pair[1].date()));
assert_eq!(ready.history().range_days(), 30);
```

Add exact known-zero, unavailable, and partial fixtures. Assert no row contains source/session/path identity and the projection remains bounded when the query test fixture contains more than 30 points.

- [ ] **Step 2: Run and verify RED**

Run: `cargo +1.97.0 test -p tokenmaster-desktop --test history_projection_contract --locked`

Expected: compile failure because History projection types do not exist.

- [ ] **Step 3: Implement the minimal projection**

Map the history envelope overview and series into owned scalar values. Reverse the already chronological daily series for newest-first table presentation, cap at 30, calculate chart maxima, copy exact date fields, and preserve public freshness/quality codes. A missing payload yields unavailable values and an empty model; a ready empty payload yields legitimate zero overview and empty rows.

- [ ] **Step 4: Run projection and presentation tests**

Run: `cargo +1.97.0 test -p tokenmaster-desktop --test history_projection_contract --test presentation_contract --locked`

Expected: pass with zero failures.

- [ ] **Step 5: Commit**

```powershell
git add -- crates/desktop/src/history.rs crates/desktop/src/lib.rs crates/desktop/src/presentation.rs crates/desktop/src/dashboard.rs crates/desktop/tests/history_projection_contract.rs crates/desktop/tests/support/dashboard_fixture.rs crates/desktop/tests/support/mod.rs
git commit -m "feat(desktop): project bounded usage history"
```

### Task 5: Replace the Slint History placeholder

**Files:**
- Modify: `crates/desktop/ui/models.slint`
- Create: `crates/desktop/ui/views/history-view.slint`
- Modify: `crates/desktop/ui/main.slint`
- Modify: `crates/desktop/src/ui.rs`
- Modify: `crates/desktop/tests/ui_contract.rs`

**Interfaces:**
- Produces Slint `HistoryDayRow` with bounded formatted scalar strings and chart ratios.
- Produces `MainWindow.history-visible` and History overview/model properties.
- Adds no callback and performs no query on route selection.

- [ ] **Step 1: Write the failing compiled UI test**

```rust
window.invoke_select_route("history".into());
assert!(window.get_history_visible());
assert!(!window.get_dashboard_visible());
assert_eq!(window.get_history_day_rows().row_count(), 30);
assert_eq!(window.get_history_range_label(), "Jun 17 – Jul 16, 2026");
assert_eq!(component_address, shell.window() as *const _);
```

Also assert initial/unavailable History has zero rows and `—` metrics, and verify the production source no longer routes `history` through `RouteState`.

- [ ] **Step 2: Run and verify RED**

Run: `cargo +1.97.0 test -p tokenmaster-desktop --test ui_contract --locked`

Expected: compile failure because History Slint properties do not exist.

- [ ] **Step 3: Implement the History view and binding**

Create a responsive software-rendered view with exact range/evidence header, overview metrics, bounded trend bars, and a scrollable daily table. Use `UiTokens`, accessible roles/labels, semantic unavailable/partial text, and the same narrow/wide breakpoint style as Dashboard. In `main.slint`, add `history-visible` and exclude it from the generic `RouteState` condition. In `ui.rs`, replace the complete History model during `apply_projection`.

- [ ] **Step 4: Run the focused desktop package**

Run: `cargo +1.97.0 test -p tokenmaster-desktop --locked`

Expected: pass with zero failures.

- [ ] **Step 5: Commit**

```powershell
git add -- crates/desktop/ui/models.slint crates/desktop/ui/views/history-view.slint crates/desktop/ui/main.slint crates/desktop/src/ui.rs crates/desktop/tests/ui_contract.rs
git commit -m "feat(ui): render real bounded history route"
```

### Task 6: Synchronize project truth and run closeout gates

**Files:**
- Modify: `spec/SPECIFICATION.md` only if the implementation clarifies a normative invariant
- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/API_CONTRACT.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `docs/FEATURE_PARITY.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/CHANGELOG.md`
- Modify: `docs/PROJECT_HISTORY.md`

**Interfaces:**
- Records P3-D.1 as a bounded 30-day History slice.
- Keeps P3-D, range controls, Sessions, parity, M0, packaging, and release incomplete.

- [ ] **Step 1: Update traceability and parity truth**

Record the implemented History route evidence under TM-FUNC-003/004 and TM-UI-001/002. Keep WMT trend, refresh, and ccusage daily-report rows `partial` because interactive ranges, filters, JSON, and complete P3-D remain open.

- [ ] **Step 2: Update current state, handoff, roadmap, changelog, and history**

State the exact next slice: Sessions page/detail. Do not record a tracked current commit hash.

- [ ] **Step 3: Run source and whitespace audits**

Run:

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
git diff --check
rg -n "TODO|TBD|placeholder" docs/superpowers/specs/2026-07-19-tokenmaster-history-route-design.md docs/superpowers/plans/2026-07-19-tokenmaster-history-route.md
```

Expected: clean-root and diff checks pass; only intentional prose uses of `placeholder` are present.

- [ ] **Step 4: Run the baseline quality gate**

```powershell
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

Expected: every command exits zero. If the full workspace gate fails, report the first causal failure and do not claim P3-D.1 complete.

- [ ] **Step 5: Audit task-owned processes and commit**

Confirm no task-owned `cargo.exe`, `rustc.exe`, or `TokenMaster.exe` remains. Then stage only the documentation changed by this task and commit:

```powershell
git commit -m "docs: close bounded history route"
```
