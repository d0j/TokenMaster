# TokenMaster P2-D Quota History Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox
> (`- [ ]`) syntax for tracking.

**Status:** inline execution in progress after spec-coverage, placeholder, type-flow,
scope, authority-boundary, and restart-state self-review; Tasks 1-6 complete, Task 7
next

**Goal:** Build the provider-neutral quota history data core that preserves scheduled,
early, repeated, and manual/banked full resets without inventing provider limits.

**Architecture:** Replace the M0 floating-point quota placeholder with fixed-point
domain values, evaluate samples in a pure `tokenmaster-quota` crate, and persist
quota-owned schema-v10 state in the existing bundled SQLite database. Quota
publication and cursors use an independent revision and immutable query envelope;
usage generations, provider transport, benefit inventory, reminders, UI, CLI/MCP,
and activation remain outside this plan.

**Tech Stack:** Rust 1.97, edition 2024, `sha2 = 0.11.0`, bundled SQLite through
`rusqlite = 0.40.1` / `libsqlite3-sys = 0.38.1`, existing synchronous
`tokenmaster-domain`, `tokenmaster-store`, and `tokenmaster-query` patterns.

## Global Constraints

- Work on `cx/tokenmaster-product-architecture`; do not push or package without
  explicit user direction.
- Use red/green TDD for every behavior change and commit every task independently.
- Quota ratios are integer parts per million in `0..=1_000_000`; no floating point.
- Never hard-code a five-hour, weekly, zero-used, or full-remaining provider rule.
- Never derive provider capacity from local tokens, sessions, cost, tasks, or time.
- Provider/account/workspace/window/unit/epoch values are bounded ASCII IDs.
- Opaque observation/epoch/transition/scope IDs are exact 32-byte values with redacted
  `Debug`.
- No network, browser, filesystem, environment, async runtime, timer, UI, plugin,
  credential, CLI/MCP, or mutation authority is added by this plan.
- All quota collections, SQL statements, transactions, maintenance pages, and public
  pages are bounded.
- Do not persist raw provider payloads, URLs, headers, cookies, credentials, prompts,
  responses, reasoning, commands, source content, or absolute paths.
- The baseline gate remains:

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
```

---

### Task 1: Exact provider-neutral quota domain values

**Files:**

- Replace: `crates/domain/src/quota.rs`
- Modify: `crates/domain/src/lib.rs`
- Create: `crates/domain/tests/quota_contract.rs`
- Modify: `crates/domain/tests/state_contract.rs`

**Interfaces:**

- Produces:
  - `QuotaAccountId`, `QuotaWorkspaceId`, `QuotaWindowId`, `QuotaUnitId`,
    `QuotaProviderEpochId`
  - `QuotaObservationId([u8; 32])`
  - `QuotaScope`, `QuotaWindowKey`, `QuotaRatio`, `QuotaUnits`
  - `QuotaWindowDefinition`, `QuotaSample`
  - `QuotaWindowSemantics`, `QuotaPresentationDirection`, `QuotaConfidence`,
    `QuotaSampleQuality`, `QuotaEvidenceSource`, `QuotaResetEvidence`
  - `QuotaResetThresholds`, `QuotaError`, and constructor-parts structs
- Removes: public `QuotaTarget` and its `f64 used_ratio`.

- [x] **Step 1: Write failing quota value contracts**

Create `crates/domain/tests/quota_contract.rs` with fixtures that call the desired API:

```rust
use tokenmaster_domain::{
    QuotaAccountId, QuotaConfidence, QuotaEvidenceSource, QuotaObservationId,
    QuotaPresentationDirection, QuotaProviderEpochId, QuotaRatio, QuotaResetEvidence,
    QuotaResetThresholds, QuotaSample, QuotaSampleParts, QuotaSampleQuality, QuotaScope,
    QuotaUnitId, QuotaUnits, QuotaWindowDefinition, QuotaWindowDefinitionParts,
    QuotaWindowId, QuotaWindowKey, QuotaWindowSemantics, QuotaWorkspaceId,
    UsageProviderId,
};

fn window_key() -> QuotaWindowKey {
    QuotaWindowKey::new(
        QuotaScope::new(
            UsageProviderId::new("codex").expect("provider"),
            QuotaAccountId::new("personal").expect("account"),
            Some(QuotaWorkspaceId::new("default").expect("workspace")),
        ),
        QuotaWindowId::new("weekly").expect("window"),
    )
}

#[test]
fn ratios_are_exact_parts_per_million() {
    assert_eq!(QuotaRatio::new(840_000).expect("ratio").parts_per_million(), 840_000);
    assert!(QuotaRatio::new(1_000_001).is_err());
}

#[test]
fn fixed_window_definition_accepts_provider_thresholds() {
    let definition = QuotaWindowDefinition::new(QuotaWindowDefinitionParts {
        key: window_key(),
        revision: 1,
        label_key: "quota.weekly".to_owned(),
        presentation: QuotaPresentationDirection::Used,
        semantics: QuotaWindowSemantics::Fixed,
        nominal_duration_seconds: Some(603_900),
        reset_thresholds: Some(
            QuotaResetThresholds::new(
                Some(QuotaRatio::new(50_000).expect("used floor")),
                Some(QuotaRatio::new(950_000).expect("remaining ceiling")),
                Some(QuotaRatio::new(500_000).expect("minimum drop")),
            )
            .expect("thresholds"),
        ),
    })
    .expect("definition");
    assert_eq!(definition.revision(), 1);
    assert_eq!(definition.nominal_duration_seconds(), Some(603_900));
}

#[test]
fn sample_preserves_ratio_only_truth_without_capacity() {
    let sample = QuotaSample::new(QuotaSampleParts {
        key: window_key(),
        observation_id: QuotaObservationId::from_bytes([7; 32]),
        observed_at_ms: 1_000,
        fresh_until_ms: 2_000,
        stale_after_ms: 5_000,
        provider_epoch_id: Some(QuotaProviderEpochId::new("epoch-17").expect("epoch")),
        used_ratio: Some(QuotaRatio::new(840_000).expect("used")),
        remaining_ratio: Some(QuotaRatio::new(160_000).expect("remaining")),
        units: None,
        advertised_resets_at_ms: Some(10_000),
        quality: QuotaSampleQuality::Authoritative,
        source: QuotaEvidenceSource::ProviderLocal,
        confidence: QuotaConfidence::High,
        reset_evidence: QuotaResetEvidence::None,
        reset_occurred_at_ms: None,
    })
    .expect("sample");
    assert!(sample.units().is_none());
    assert_eq!(
        sample.used_ratio().expect("used").parts_per_million(),
        840_000
    );
}

#[test]
fn absolute_units_are_optional_bounded_and_coherent() {
    let units = QuotaUnits::new(
        QuotaUnitId::new("provider_units").expect("unit"),
        Some(84),
        Some(16),
        Some(100),
    )
    .expect("units");
    assert_eq!(units.capacity(), Some(100));
    assert!(QuotaUnits::new(
        QuotaUnitId::new("provider_units").expect("unit"),
        Some(101),
        None,
        Some(100),
    )
    .is_err());
}
```

Add adversarial tests for empty/oversized/unsafe IDs, zero definition revision,
non-fixed thresholds, empty thresholds, invalid time order, invalid exact reset time,
empty samples, redacted observation `Debug`, and serde round-trips that preserve
`None` rather than zero.

- [x] **Step 2: Run the focused test and verify RED**

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-domain --test quota_contract --locked
```

Expected: compilation fails because the new quota types are not exported.

- [x] **Step 3: Implement the minimal bounded types**

Replace `crates/domain/src/quota.rs` with private validated string wrappers and the
exact constructors exercised above. Use this common ID validator:

```rust
fn valid_quota_id(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}
```

Implement `QuotaObservationId::Debug` as exactly
`QuotaObservationId([redacted])`. Derive `Serialize`/`Deserialize` only through
validated custom deserialization for public IDs, ratios, units, definitions, and
samples. `QuotaSample::new` must enforce:

```rust
parts.observed_at_ms > 0
    && parts.observed_at_ms <= parts.fresh_until_ms
    && parts.fresh_until_ms <= parts.stale_after_ms
```

It must also require at least one ratio, units value, advertised reset time, provider
epoch ID, or explicit reset evidence. `reset_occurred_at_ms` is valid only with
non-`None` reset evidence and within `1..=observed_at_ms`.

`QuotaUnits::new` requires at least one numeric field and rejects any used/remaining
value above a present capacity. `QuotaResetThresholds::new` requires at least one
post-reset used/remaining boundary. `QuotaWindowDefinition::new` accepts thresholds
only for `Fixed`.

Export all new types from `crates/domain/src/lib.rs`. Remove the quota-specific tests
and `QuotaTarget` import from `state_contract.rs`.

- [x] **Step 4: Run focused tests and strict crate Clippy**

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-domain --test quota_contract --locked
cargo +1.97.0 test -p tokenmaster-domain --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-domain --all-targets --locked
```

Expected: all pass with zero warnings.

- [x] **Step 5: Commit**

```powershell
git add -- crates/domain/src/quota.rs crates/domain/src/lib.rs crates/domain/tests/quota_contract.rs crates/domain/tests/state_contract.rs
git commit -m "feat(domain): add exact quota observations"
```

---

### Task 2: Pure quota detector and deterministic identities

**Files:**

- Create: `crates/quota/Cargo.toml`
- Create: `crates/quota/src/lib.rs`
- Create: `crates/quota/src/identity.rs`
- Create: `crates/quota/src/detector.rs`
- Create: `crates/quota/tests/identity_contract.rs`
- Create: `crates/quota/tests/detector_contract.rs`
- Modify: `Cargo.toml`

**Interfaces:**

- Consumes: Task 1 domain types.
- Produces:

```rust
pub fn evaluate_sample(
    definition: &QuotaWindowDefinition,
    current: Option<&QuotaEpochState>,
    previous: Option<&QuotaSample>,
    sample: &QuotaSample,
    next_transition_sequence: u64,
) -> Result<QuotaEvaluation, QuotaError>;
```

`QuotaEvaluation` variants are `Started`, `Duplicate`, `Stale`, `Advanced`,
`AllowanceChanged`, and `Reset`. Visible variants carry a complete next
`QuotaEpochState`; transition variants additionally carry one immutable
`QuotaTransition`.

- [x] **Step 1: Write failing detector fixtures**

Cover exact provider epoch reset, explicit provider/local/manual reset, provider
threshold scheduled reset, early reset, inferred unknown reset, repeated reset,
allowance change with/without reset, rolling recovery rejection, drop-only rejection,
duplicate ID/content, duplicate ID/conflicting content, stale time, scope/window
mismatch, quality/confidence gates, absent reset time interval, and sequence overflow.

Use provider-defined nonstandard fixtures such as a 603,900-second fixed window and
post-reset thresholds of 5% used / 95% remaining. No fixture may assume five hours or
seven days.

- [x] **Step 2: Verify RED**

```powershell
cargo +1.97.0 test -p tokenmaster-quota --locked
```

Expected: package is absent.

- [x] **Step 3: Implement the pure crate**

Add workspace member `crates/quota` with only:

```toml
[dependencies]
sha2.workspace = true
tokenmaster-domain = { path = "../domain" }
```

Hash normalized length-prefixed fields with SHA-256 and distinct domain tags:
`tokenmaster.quota.scope.v1`, `tokenmaster.quota.epoch.v1`, and
`tokenmaster.quota.transition.v1`. Do not use `Debug`, JSON, architecture-dependent
integer bytes, or map iteration as identity input.

Detection precedence and confidence follow the design exactly. Store maximum used
ratio and matching absolute used units only when they are available and comparable.

- [x] **Step 4: Verify crate tests, privacy, and dependency closure**

```powershell
cargo +1.97.0 test -p tokenmaster-quota --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-quota --all-targets --locked
cargo tree -p tokenmaster-quota --locked
```

Expected: only standard library, `sha2`, and `tokenmaster-domain` in the production
closure; all tests pass.

- [x] **Step 5: Commit**

```powershell
git add -- Cargo.toml Cargo.lock crates/quota
git commit -m "feat(quota): add deterministic reset detector"
```

---

### Task 3: Strict schema v10 and exact v9 migration

**Files:**

- Create: `crates/store/src/usage/quota_schema.rs`
- Modify: `crates/store/src/usage/mod.rs`
- Modify: `crates/store/src/usage/schema.rs`
- Modify: `crates/store/src/usage/migration.rs`
- Modify: `crates/store/src/usage/query.rs`
- Create: `crates/store/tests/quota_schema_contract.rs`
- Modify: `crates/store/tests/usage_schema_contract.rs`
- Modify: `crates/store/tests/pricing_rollup_contract.rs`

**Interfaces:**

- Produces `USAGE_SCHEMA_VERSION = 10` and exact quota table/index/trigger contracts.
- Fresh and v9-migrated archives contain one `quota_state(singleton_id=1, revision=0)`.

- [x] **Step 1: Write failing fresh/migration/malformed schema tests**

Assert exact `STRICT` tables, 32-byte opaque ID checks, enum checks, foreign keys,
indexes for current scope/window, transition sequence, sample retention, and
UPDATE-protected retained-history triggers. Store-owned DELETE remains reserved for
the bounded retention task. Assert an exact v9 database migrates without changing
usage counts, aggregate generations, price rows, or usage dataset generation.

Inject a failure after quota table creation and prove the entire migration rolls back
to exact v9 with no quota objects.

- [x] **Step 2: Verify RED**

```powershell
cargo +1.97.0 test -p tokenmaster-store --test quota_schema_contract --locked
```

Expected: schema version remains 9 and quota tables are absent.

- [x] **Step 3: Implement schema and migration**

`quota_schema.rs` owns `V10_QUOTA_SCHEMA`, table/index contracts, and retained-history
UPDATE guards. `migrate_schema` runs the v9-to-v10 step only after exact v9 validation,
sets `user_version=10` inside the same immediate transaction, then runs `validate_v10`.

After the exact-v9 precondition validation, the migration transaction may execute only
quota DDL, the empty quota-state seed, the version update, and v10 validation. It may
not rewrite or reclassify usage or price rows.

- [x] **Step 4: Verify all store schema/migration tests**

```powershell
cargo +1.97.0 test -p tokenmaster-store --test quota_schema_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test usage_schema_contract --locked
cargo +1.97.0 test -p tokenmaster-store --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-store --all-targets --locked
```

The complete global clean-root, formatting, warnings-as-errors workspace Clippy, and
locked workspace test/doctest baseline also passes.

- [x] **Step 5: Commit**

```powershell
git add -- crates/store/src/usage/quota_schema.rs crates/store/src/usage/mod.rs crates/store/src/usage/schema.rs crates/store/src/usage/migration.rs crates/store/src/usage/query.rs crates/store/tests/quota_schema_contract.rs crates/store/tests/usage_schema_contract.rs crates/store/tests/pricing_rollup_contract.rs
git commit -m "feat(store): add quota schema v10"
```

---

### Task 4: Transactional quota observation application

**Files:**

- Create: `crates/store/src/usage/quota_write.rs`
- Create: `crates/store/src/usage/quota_types.rs`
- Modify: `crates/store/src/usage/mod.rs`
- Modify: `crates/store/src/lib.rs`
- Modify: `crates/store/src/usage/migration.rs`
- Modify: `crates/store/Cargo.toml`
- Create: `crates/store/tests/quota_write_contract.rs`

**Interfaces:**

```rust
impl UsageStore {
    pub fn apply_quota_observation(
        &mut self,
        definition: &QuotaWindowDefinition,
        sample: &QuotaSample,
    ) -> Result<QuotaApplyResult, StoreError>;
}
```

`QuotaApplyStatus` is `Started`, `Duplicate`, `Stale`, `Advanced`,
`AllowanceChanged`, or `Reset`. `QuotaApplyResult` exposes status, quota revision,
window transition sequence, and optional transition ID without exposing SQL or private
scope IDs.

- [x] **Step 1: Write failing transactional contracts**

Cover first sample, no-op duplicate, stale sample, normal advance, standalone
allowance change, reset, reset plus allowance change, two repeated resets, account
switch isolation, reopen continuity, deterministic retry, and injected rollback after
sample/epoch/transition/current/revision boundaries.

- [x] **Step 2: Verify RED**

```powershell
cargo +1.97.0 test -p tokenmaster-store --test quota_write_contract --locked
```

- [x] **Step 3: Implement one immediate transaction**

Load only one window's definition/current epoch/last sample/next sequence. Call
`tokenmaster_quota::evaluate_sample`, insert normalized immutable values, update the
current projection, and advance `quota_state.revision` exactly once for every visible
non-duplicate/non-stale result. All writes and the revision advance commit or roll back
together. Observation identity is global and content-stable, definition identity is
immutable per revision, transition/SQLite capacity is checked, and an epoch/current/
last-sample mismatch fails closed rather than being silently repaired.

- [x] **Step 4: Verify focused and store suites**

```powershell
cargo +1.97.0 test -p tokenmaster-store --test quota_write_contract --locked
cargo +1.97.0 test -p tokenmaster-store --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-store --all-targets --locked
```

- [x] **Step 5: Commit**

```powershell
git add -- crates/store/Cargo.toml crates/store/src/usage/quota_write.rs crates/store/src/usage/quota_types.rs crates/store/src/usage/migration.rs crates/store/src/usage/mod.rs crates/store/src/lib.rs crates/store/tests/quota_write_contract.rs Cargo.lock
git commit -m "feat(store): persist immutable quota transitions"
```

---

### Task 5: Bounded retention, restart, and fault evidence

**Files:**

- Modify: `crates/store/src/usage/quota_write.rs`
- Create: `crates/store/src/usage/quota_maintenance.rs`
- Modify: `crates/store/src/usage/quota_types.rs`
- Modify: `crates/store/src/usage/migration.rs`
- Modify: `crates/store/src/usage/mod.rs`
- Modify: `crates/store/src/lib.rs`
- Create: `crates/store/tests/quota_retention_contract.rs`

**Interfaces:**

```rust
impl UsageStore {
    pub fn maintain_quota_history_page(
        &mut self,
        window: &QuotaWindowKey,
        page_size: u16,
    ) -> Result<QuotaMaintenanceResult, StoreError>;
}
```

Page size is `1..=256`. The result exposes examined/deleted/remaining counts only.

- [x] **Step 1: Write failing retention tests**

Generate 10,000 redundant polls, 513 meaningful samples, 257 resets, protected
first/last/max/pre/post samples, restart between every detector boundary, maintenance
fault rollback, and sequence/revision overflow.

- [x] **Step 2: Verify RED**

```powershell
cargo +1.97.0 test -p tokenmaster-store --test quota_retention_contract --locked
```

- [x] **Step 3: Implement bounded compaction**

Use fixed keyset deletes of unprotected redundant samples only. Never scan another
window and never delete current, unresolved, first, last, maximum-use, pre-reset, or
post-reset evidence. Preserve all transitions until a later aggregate contract exists;
therefore Task 5 may report over-default transition backlog but must remain below the
hard cap or fail the applying write closed.

- [x] **Step 4: Verify**

```powershell
cargo +1.97.0 test -p tokenmaster-store --test quota_retention_contract --locked
cargo +1.97.0 test -p tokenmaster-store --locked
```

- [x] **Step 5: Commit**

```powershell
git add -- crates/store/src/lib.rs crates/store/src/usage/migration.rs crates/store/src/usage/mod.rs crates/store/src/usage/quota_maintenance.rs crates/store/src/usage/quota_types.rs crates/store/src/usage/quota_write.rs crates/store/tests/quota_retention_contract.rs
git commit -m "feat(store): bound quota history retention"
```

---

### Task 6: Defensive quota read snapshots and keyset history

**Files:**

- Create: `crates/store/src/usage/query/quota.rs`
- Modify: `crates/quota/src/detector.rs`
- Modify: `crates/quota/src/identity.rs`
- Modify: `crates/quota/src/lib.rs`
- Modify: `crates/quota/tests/detector_contract.rs`
- Modify: `crates/store/src/usage/query.rs`
- Modify: `crates/store/src/usage/mod.rs`
- Modify: `crates/store/src/lib.rs`
- Modify: `crates/store/src/usage/quota_write.rs`
- Create: `crates/store/tests/quota_query_contract.rs`

**Interfaces:**

```rust
impl UsageReadStore {
    pub fn capture_quota_windows(
        &mut self,
        query: QuotaCurrentQuery,
    ) -> Result<QuotaCurrentCapture, StoreError>;

    pub fn capture_quota_transitions(
        &mut self,
        query: QuotaTransitionPageQuery,
    ) -> Result<QuotaTransitionPageCapture, StoreError>;
}
```

Current query accepts at most 32 exact window keys. Transition page accepts one exact
window, exact optional expected quota revision, optional opaque cursor, `1..=256`
page size, and a maximum two-second deadline.

- [x] **Step 1: Write failing query/value/index tests**

Cover empty/current/multiple scopes, exact revision binding, 256+1 lookahead,
descending sequence cursor, changed filter/revision rejection, missing window, deadline
cleanup, query-only behavior, owned values, redacted cursor/ID `Debug`, and real
`EXPLAIN QUERY PLAN` index seeks. Assert quota SQL contains no usage/price table.

- [x] **Step 2: Verify RED**

```powershell
cargo +1.97.0 test -p tokenmaster-store --test quota_query_contract --locked
```

- [x] **Step 3: Implement fixed read SQL**

Add no caller-defined SQL, sort, expression, or column selection. Capture quota
revision and rows in one deferred transaction. Remove the progress handler before
every success/error return.

Critical review additionally requires validated transition restoration, reconciliation
of duplicated current/transition projections with boundary samples, post-open drift
rejection, and rejection of a capture that completes after the total deadline even if
no individual SQLite statement crossed the progress callback interval.

- [x] **Step 4: Verify**

```powershell
cargo +1.97.0 test -p tokenmaster-quota --locked
cargo +1.97.0 test -p tokenmaster-store --test quota_query_contract --locked
cargo +1.97.0 test -p tokenmaster-store --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-quota --all-targets --locked
cargo +1.97.0 clippy -p tokenmaster-store --all-targets --locked
```

- [x] **Step 5: Commit**

```powershell
git add -- crates/quota/src/detector.rs crates/quota/src/identity.rs crates/quota/src/lib.rs crates/quota/tests/detector_contract.rs crates/store/src/usage/query/quota.rs crates/store/src/usage/query.rs crates/store/src/usage/mod.rs crates/store/src/lib.rs crates/store/src/usage/quota_write.rs crates/store/tests/quota_query_contract.rs
git commit -m "feat(store): add bounded quota snapshots"
```

---

### Task 7: Immutable quota query facade

**Files:**

- Create: `crates/query/src/quota.rs`
- Create: `crates/query/src/quota_identity.rs`
- Modify: `crates/query/src/lib.rs`
- Modify: `crates/query/src/service.rs`
- Create: `crates/query/tests/quota_value_contract.rs`
- Create: `crates/query/tests/quota_service_contract.rs`

**Interfaces:**

```rust
impl<C: QueryClock> QueryService<C> {
    pub fn quota_windows(
        &mut self,
        request: QuotaCurrentRequest,
    ) -> Result<QuotaEnvelope<QuotaCurrentSnapshot>, QueryError>;

    pub fn quota_transitions(
        &mut self,
        request: QuotaTransitionPageRequest,
    ) -> Result<QuotaEnvelope<QuotaTransitionPage>, QueryError>;
}
```

`QuotaQueryHeader` carries `SnapshotGeneration`, `QuotaRevision`, generated/data-
through time, `QueryFreshness`, `QueryQuality`, exact window filters, and bounded
warnings. It deliberately has no usage `DatasetIdentity`.

- [ ] **Step 1: Write failing immutable facade tests**

Cover authoritative/fresh, aging, stale, unavailable, partial, conflict, scheduled,
early, manual/banked, unknown, exact time, bounded interval, ratio-only, unit-bearing,
allowance change, repeated sequence, opaque continuation, stale revision, failed-call
snapshot-generation neutrality, and public Debug/privacy.

- [ ] **Step 2: Verify RED**

```powershell
cargo +1.97.0 test -p tokenmaster-query --test quota_value_contract --locked
cargo +1.97.0 test -p tokenmaster-query --test quota_service_contract --locked
```

- [ ] **Step 3: Implement mapping and service methods**

Freshness uses each sample's exact `fresh_until_ms` / `stale_after_ms`; no 20-minute
usage TTL is reused. Aggregate current-window quality is the strongest truthful
downgrade across selected windows. Allocate `SnapshotGeneration` only after the store
capture and public mapping both succeed.

- [ ] **Step 4: Verify**

```powershell
cargo +1.97.0 test -p tokenmaster-query --test quota_value_contract --locked
cargo +1.97.0 test -p tokenmaster-query --test quota_service_contract --locked
cargo +1.97.0 test -p tokenmaster-query --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-query --all-targets --locked
```

- [ ] **Step 5: Commit**

```powershell
git add -- crates/query/src/quota.rs crates/query/src/quota_identity.rs crates/query/src/lib.rs crates/query/src/service.rs crates/query/tests/quota_value_contract.rs crates/query/tests/quota_service_contract.rs
git commit -m "feat(query): expose immutable quota history"
```

---

### Task 8: Scale, resource, privacy, and project-truth closure

**Files:**

- Create: `crates/quota/tests/adversarial_contract.rs`
- Create: `crates/query/tests/quota_scale_contract.rs`
- Modify: `crates/query/tests/resource_contract.rs`
- Create: `scripts/audit-quota-network.ps1`
- Modify: `spec/SPECIFICATION.md`
- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/API_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/FEATURE_PARITY.md`
- Modify: `docs/CHANGELOG.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: `docs/AUDIT_AND_MASTER_PLAN.md`
- Modify: this plan and its design status

- [ ] **Step 1: Add adversarial and release-scale gates**

The ignored release gate must cover at least 32 windows, 1,000 transitions, 10,000
redundant polls, scheduled/early/repeated/manual resets, restart, current reads,
256-row cursor history, bounded maintenance, and current/legacy usage data coexistence.
Quota writes and reads must stay below one second on the reference machine; repeated
current/history/switch/reopen cycles must preserve Windows private-memory, handles,
threads, USER, and GDI high-water bounds.

- [ ] **Step 2: Add production authority audit**

`scripts/audit-quota-network.ps1` builds the release `tokenmaster-quota`,
`tokenmaster-store`, and `tokenmaster-query` closure; rejects HTTP/browser/async
clients; scans production source and release libraries for private endpoint, cookie,
browser automation, shell, and arbitrary network signatures.

- [ ] **Step 3: Run the complete gate**

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
$arguments = @(
  '+1.97.0', 'test', '-p', 'tokenmaster-query', '--test',
  'quota_scale_contract', '--release', '--locked', '--', '--ignored', '--nocapture'
)
& cargo @arguments
pwsh -NoProfile -File scripts\audit-quota-network.ps1 -RepositoryRoot (Get-Location).Path
git diff --check
```

- [ ] **Step 4: Update project truth**

Record exact schema, bounds, gates, measured receipts, and the next honest blocker.
Never place the current commit hash in tracked documents. Do not claim Codex quota
transport, banked inventory, reminders, UI, CLI/MCP, M0 acceptance, package, signing,
or release.

- [ ] **Step 5: Commit**

```powershell
git add -- crates/quota/tests/adversarial_contract.rs crates/query/tests/quota_scale_contract.rs crates/query/tests/resource_contract.rs scripts/audit-quota-network.ps1 spec docs
git commit -m "test(quota): close quota core acceptance"
```

P2-D quota history core completion authorizes only the next separate data contours:
permitted Codex quota transport and banked-benefit inventory/reminder policy.

## Plan self-review result

- Every requirement in the approved design maps to one of Tasks 1-8.
- The task order has no dependency cycle: domain values, pure evaluation, schema,
  writes, retention, reads, public query values, then acceptance evidence.
- Quota transport, benefit inventory, reminders, notification delivery, UI,
  CLI/MCP, and provider mutation remain explicitly outside this plan.
- All public values used by later tasks are produced by an earlier task; quota
  revision and identity remain independent from usage dataset generations.
- The plan contains no unfinished markers, deferred implementation phrase, assumed
  five-hour/seven-day constant, or caller-defined authority surface.
- Each task has an independent RED/GREEN validator and an intentional commit
  boundary. Task 8 repeats the complete repository gate before project-truth
  closure.
