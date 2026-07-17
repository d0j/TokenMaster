# TokenMaster P3-C Quota-First Dashboard Implementation Plan

> Execute task by task with focused red/green tests and milestone commits. Keep the
> exact authority, privacy, and bounds from the approved P3-C design.

**Goal:** Render a truthful six-section quota-first Dashboard from one live immutable
product snapshot, including bounded all-current quota/benefit discovery, without UI
queries, polling, private identities, or unbounded retention.

**Architecture:** Add explicit store/query overview reads, publish them through the
existing reducer/controller, map them into one bounded `DesktopDashboardProjection`,
and apply semantic Slint card models through the existing capacity-one bridge.

**Stack:** Rust 1.97, bundled SQLite, Slint 1.17 software renderer, existing fixed-point
pricing/query/product/runtime contracts, PowerShell/Pester structural audits.

---

## Task 1: Freeze all-current quota discovery

**Files:**
- Modify: `crates/store/src/usage/query/quota.rs`
- Modify: `crates/store/src/usage/query/mod.rs`
- Modify: `crates/store/src/usage/mod.rs`
- Test: `crates/store/tests/quota_query_contract.rs`

1. Write failing tests proving exact-empty remains empty, overview returns every
   ordered current window under one revision, 32 passes, 33 fails, and corruption/
   deadline leaves no partial capture.
2. Add `QuotaOverviewQuery` and a one-transaction ordered key discovery plus exact
   current-window restoration.
3. Run:
   `cargo +1.97.0 test -p tokenmaster-store --test quota_query_contract --locked`
4. Commit: `feat(store): discover current quota windows`.

## Task 2: Expose query quota overview

**Files:**
- Modify: `crates/query/src/quota.rs`
- Modify: `crates/query/src/service.rs`
- Modify: `crates/query/src/lib.rs`
- Test: `crates/query/tests/quota_service_contract.rs`
- Test: `crates/query/tests/quota_scale_contract.rs`

1. Write failing mapping/privacy tests for `QuotaOverviewRequest` and the existing
   immutable `QuotaEnvelope<QuotaCurrentSnapshot>`.
2. Preserve exact-filter API behavior and add `QueryService::quota_overview`.
3. Prove ordered filters, freshness/quality/warnings, revision binding, 32/33 bounds,
   stable errors, and no identity in Debug/error text.
4. Run focused package tests with `RUSTFLAGS=-Dwarnings`.
5. Commit: `feat(query): expose quota overview`.

## Task 3: Freeze all-current benefit overview

**Files:**
- Modify: `crates/store/src/usage/query/benefit.rs`
- Modify: `crates/store/src/usage/query/mod.rs`
- Modify: `crates/store/src/usage/mod.rs`
- Test: `crates/store/tests/benefit_query_contract.rs`
- Test: `crates/store/tests/benefit_retention_contract.rs`

1. Write failing tests for one-transaction scope/lot capture, opaque ordering, FEFO
   order, profiles/nearest due, 32-scope and 256-lot maxima, plus-one rejection,
   revision races, corruption, and deadline cleanup.
2. Add `BenefitOverviewQuery`, scope captures, and validators reusing exact-scope
   restoration helpers.
3. Run the focused store benefit suites.
4. Commit: `feat(store): discover current benefits`.

## Task 4: Expose query benefit overview

**Files:**
- Modify: `crates/query/src/benefit.rs`
- Modify: `crates/query/src/service.rs`
- Modify: `crates/query/src/lib.rs`
- Test: `crates/query/tests/benefit_query_contract.rs`
- Test: `crates/query/tests/benefit_scale_contract.rs`

1. Write failing tests for immutable multi-scope output, separate benefit kinds,
   available banked-reset counts, conservative nearest expiry, reminder coverage,
   freshness/quality, overflow, and redaction.
2. Add `BenefitOverviewRequest`, `BenefitOverviewSnapshot`, and service mapping.
3. Keep exact-scope current/history APIs unchanged and activation-free.
4. Run focused query benefit suites with warnings denied.
5. Commit: `feat(query): expose benefit overview`.

## Task 5: Publish overview payloads through product/controller

**Files:**
- Modify: `crates/product/src/snapshot.rs`
- Modify: `crates/product/src/reducer.rs`
- Modify: `crates/product/src/route.rs`
- Modify: `crates/desktop/src/controller.rs`
- Test: `crates/product/tests/reducer_contract.rs`
- Test: `crates/product/tests/route_contract.rs`
- Test: `crates/desktop/tests/controller_contract.rs`

1. Write failing compatibility, stale-attempt, sibling-failure, and dashboard-route
   tests for overview payloads.
2. Update the overview plan to request 12 activity/session rows and explicit quota/
   benefit overview calls.
3. Preserve one query worker, one reducer, complete-attempt publication, one latest
   snapshot, and exact runtime observation ordering.
4. Run focused product and desktop controller tests.
5. Commit: `feat(desktop): publish dashboard overview`.

## Task 6: Add bounded dashboard projection

**Files:**
- Add: `crates/desktop/src/dashboard.rs`
- Modify: `crates/desktop/src/presentation.rs`
- Modify: `crates/desktop/src/lib.rs`
- Test: `crates/desktop/tests/dashboard_projection_contract.rs`

1. Write failing tests for the six exact sections, all state/availability mappings,
   dynamic quota ratios/resets, separate reset credits, checked Git aggregation,
   240/12/8/12 caps, model identity mapping, and private-value exclusion.
2. Implement pure snapshot-to-dashboard mapping with checked arithmetic and stable
   semantic keys.
3. Add a 10,000-replacement retention test and a release-size mapping fixture.
4. Run focused desktop tests and strict Clippy.
5. Commit: `feat(desktop): project dashboard data`.

## Task 7: Build semantic Slint Dashboard

**Files:**
- Modify: `crates/desktop/ui/models.slint`
- Modify: `crates/desktop/ui/tokens.slint`
- Add: `crates/desktop/ui/components/section-state.slint`
- Add: `crates/desktop/ui/components/metric-value.slint`
- Add: `crates/desktop/ui/components/quota-row.slint`
- Add: `crates/desktop/ui/views/dashboard-view.slint`
- Modify: `crates/desktop/ui/main.slint`
- Modify: `crates/desktop/src/ui.rs`
- Test: `crates/desktop/tests/ui_contract.rs`
- Test: `crates/desktop/tests/bridge_event_loop_contract.rs`

1. Write failing headless UI tests for real header/dashboard values, 32 dynamic quota
   rows, unknown values, six section keys, reset-credit separation, narrow/wide
   layout, route switching, and no window recreation.
2. Implement semantic models/components and one bounded model replacement.
3. Keep non-Dashboard routes on truthful placeholders; add no timer or animation.
4. Run all desktop tests with warnings denied.
5. Commit: `feat(ui): render quota-first dashboard`.

## Task 8: Add adversarial audits and close project truth

**Files:**
- Modify: `scripts/audit-desktop-shell.ps1`
- Modify: `scripts/tests/audit-desktop-shell.Tests.ps1`
- Modify: `docs/FEATURE_PARITY.md`
- Modify: `spec/SPECIFICATION.md`
- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/API_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/CHANGELOG.md`
- Modify: `docs/PROJECT_HISTORY.md`

1. Add audit failures for empty-filter discovery drift, fixed 5h/weekly rows, seeded
   dashboard values, private IDs, UI query/runtime/SQL authority, second worker/slot/
   event, timer/animation polling, unbounded models, and diagnostic renderer drift.
2. Run desktop/application source and release audits plus all Pester contracts.
3. Record exact completed evidence and leave P3-D/P3-E, P4-P6, activation, M0,
   packaging, signing, and release explicitly unclaimed.
4. Run the baseline quality gate:

   ```powershell
   pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
   cargo +1.97.0 fmt --all -- --check
   $env:RUSTFLAGS = '-Dwarnings'
   cargo +1.97.0 clippy --workspace --all-targets --locked
   cargo +1.97.0 test --workspace --locked
   ```

5. Audit task-owned processes and Git cleanliness.
6. Commit: `docs(ui): close quota-first dashboard milestone`.
