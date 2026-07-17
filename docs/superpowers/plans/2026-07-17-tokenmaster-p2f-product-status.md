# TokenMaster P2-F Joined Product Status Implementation Plan

> Execute in order with focused red/green tests. Each task ends in a reviewable commit;
> no task claims P3 UI, P5 automation, M0 acceptance, packaging, signing, or release.

**Goal:** deliver exact durable product status and a constant-state immutable product
projection ready for the P3 Slint query worker.

**Architecture:** `tokenmaster-store` captures scalar cross-family durable status in
one defensive read transaction. `tokenmaster-query` maps it into owned public values.
The new Slint-free `tokenmaster-product` crate combines independently versioned query
and runtime snapshots without false global atomicity or retained history.

**Stack:** Rust 1.97, existing bundled SQLite/rusqlite policy, existing query/runtime
values, no new third-party dependency.

## Task 1 — Freeze store status values and validation

**Files:**
- Add: `crates/store/src/usage/query/status.rs`
- Modify: `crates/store/src/usage/query.rs`
- Modify: `crates/store/src/usage/mod.rs`
- Modify: `crates/store/src/lib.rs`
- Add: `crates/store/tests/product_status_contract.rs`

Write failing tests for zero/current/legacy usage identity; all aggregate states and
progress; independent quota/benefit/Git zero and nonzero revisions; redacted Debug;
and constructor bounds. Add owned store status types with private fields and checked
accessors. No capture method yet.

**Validator:**
`cargo +1.97.0 test -p tokenmaster-store --test product_status_contract --locked`

**Commit:** `feat(store): define bounded product status values`

## Task 2 — Add exact defensive status capture

**Files:**
- Modify: `crates/store/src/usage/query/status.rs`
- Modify: `crates/store/tests/product_status_contract.rs`

Write failing tests for one deferred transaction under a concurrent commit, fixed
state-table SQL, deadline/completed-late interruption, handler cleanup, corruption,
post-open drift, and no mutation. Implement `ProductDataStatusQuery` and
`UsageReadStore::capture_product_data_status`; validate all joined scalar invariants
inside the transaction and clear the progress handler on every path.

**Validator:** Task 1 validator plus the store read-policy tests.

**Commit:** `feat(store): capture exact product data status`

## Task 3 — Publish immutable query status

**Files:**
- Add: `crates/query/src/status.rs`
- Modify: `crates/query/src/lib.rs`
- Modify: `crates/query/src/service.rs`
- Add: `crates/query/tests/product_status_contract.rs`

Write failing tests for public schema/version/generation, independent component
identities, aggregate warning/status mapping, freshness/quality, zero-revision empty
truth, failed-call generation neutrality, older-result ordering, and Debug/privacy.
Implement `QueryService::product_data_status` with one clock sample and the common
two-second deadline.

**Validator:**
`cargo +1.97.0 test -p tokenmaster-query --test product_status_contract --locked`

**Commit:** `feat(query): expose joined product data status`

## Task 4 — Add the constant-state product projection

**Files:**
- Modify: `Cargo.toml`
- Add: `crates/product/Cargo.toml`
- Add: `crates/product/src/lib.rs`
- Add: `crates/product/src/section.rs`
- Add: `crates/product/src/snapshot.rs`
- Add: `crates/product/src/reducer.rs`
- Add: `crates/product/tests/reducer_contract.rs`

Write failing tests for checked product generation, ready/waiting/unavailable slots,
strict section ordering, equal/older coalescing, one-slot replacement, independent
fault isolation, and no persistence/UI/runtime handles. Implement a pure synchronous
reducer with one current immutable snapshot and no history.

**Validator:**
`cargo +1.97.0 test -p tokenmaster-product --test reducer_contract --locked`

**Commit:** `feat(product): add immutable snapshot reducer`

## Task 5 — Bind durable identities and route readiness

**Files:**
- Add: `crates/product/src/route.rs`
- Modify: `crates/product/src/reducer.rs`
- Modify: `crates/product/src/snapshot.rs`
- Add: `crates/product/tests/route_contract.rs`

Write failing tests for all eleven route readiness projections, aggregate rebuild
behavior, stale retained payloads, proven dataset/revision invalidation, freshness-only
retention, quota/Git section degradation, notification/activation separation, and the
eight-reason cap. Implement fixed route enums and stable reason codes; do not expose a
dynamic route/plugin UI schema.

**Validator:** product reducer and route contract tests.

**Commit:** `feat(product): derive bounded route readiness`

## Task 6 — Join runtime health without coupling ownership

**Files:**
- Add: `crates/product/src/runtime.rs`
- Modify: `crates/product/src/reducer.rs`
- Add: `crates/product/tests/runtime_status_contract.rs`

Write failing tests that usage, quota, benefit-reminder, and Git runtime faults affect
only their owned sections; pause/resume/recovery are visible; runtime generations stay
independent from durable revisions; and no runtime object/guard/callback is retained.
Map only copied count/code/lifecycle snapshots into product status.

**Validator:** product runtime-status contract plus existing runtime lifecycle tests.

**Commit:** `feat(product): join bounded runtime health`

## Task 7 — Prove latency, retention, and resource return

**Files:**
- Add: `crates/query/tests/product_status_scale_contract.rs`
- Add: `crates/product/tests/resource_contract.rs`
- Add: `scripts/audit-product-status.ps1`

Prove fixed query plan/no archive scan, large-fixture p95 below 25 ms, 10,000 reducer
updates retaining one value per slot, repeated open/capture/drop resource return, and
zero forbidden dependency/source/private-string authority. Keep deterministic tests in
the normal suite; isolate only the Windows process/resource probe if required.

**Commit:** `test(product): prove bounded status resources`

## Task 8 — Close project truth and the P2 gate

**Files:**
- Modify: `spec/SPECIFICATION.md`
- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/API_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `docs/ARCHITECTURE.md`
- Modify: `docs/AUDIT_AND_MASTER_PLAN.md`
- Modify: `docs/CHANGELOG.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/FEATURE_PARITY.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: `docs/ROADMAP.md`

Run the focused suites, status authority audit, clean-root audit, format, strict locked
workspace Clippy, and complete locked workspace tests/doctests. Review the full diff,
Git cleanliness, dependency/language footprint, and task-owned processes. Mark P2-F
complete only from passing evidence and set P3 complete desktop UI as the next slice.

**Baseline validator:**

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

**Commit:** `docs(product): close joined status contour`

