# TokenMaster Benefit Inventory and Reminder Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use
> `superpowers:executing-plans`, `superpowers:test-driven-development`,
> `superpowers:systematic-debugging`, and
> `superpowers:verification-before-completion`. Mark a checkbox only after its
> validator passes.

**Status:** in progress

**Goal:** publish the built-in Codex reset-credit inventory as strict provider-neutral
lots, preserve bounded immutable history, expose immutable read snapshots, and
schedule restart-safe in-app reminders without adding provider mutation authority.

**Design:**
`docs/superpowers/specs/2026-07-16-tokenmaster-benefit-inventory-design.md`

**Tech stack:** Rust 1.97, edition 2024, new pure `tokenmaster-benefits` crate,
existing strict Codex app-server transport, bundled SQLite schema v11, existing
constant-state scheduler/worker, and current query facade patterns. No HTTP client,
browser, shell, async runtime, credential store, or provider activation API.

## Global constraints

- Work only on `cx/tokenmaster-product-architecture`; do not push, package, or claim a
  release.
- Keep banked resets, normal quota epochs, usage credits, and temporary usage
  structurally distinct.
- Never persist or expose raw Codex credit IDs, email, title, description, frames,
  payloads, paths, or provider errors.
- Do not infer expiry, quantity, target window, capacity, value, redemption, or
  activation.
- Provider I/O completes before the shared writer lease and SQLite open.
- Use one timer/scheduler contour, never per-lot timers or threads.
- Keep activation links, intents, receipts, mutations, automatic policies, UI,
  CLI/MCP, and external plugins out of this implementation.
- Use focused red/green tests and independently reviewable checkpoint commits.

---

### Task 1: Freeze provider-neutral benefit values

**Files:**

- Create: `crates/domain/src/benefit.rs`
- Modify: `crates/domain/src/lib.rs`
- Create: `crates/domain/tests/benefit_contract.rs`

**RED/GREEN:**

- [x] Add opaque redacted lot and observation IDs.
- [x] Add strict scope, kind, state, source, confidence, detail, target, expiry, lot,
  inventory observation, notification channel, and reminder profile values.
- [x] Enforce 64 lots, positive ordered times, unique lot IDs, positive quantities,
  strict IDs/label keys/time-zone IDs, exact/bounded expiry coherence, 8 unique
  thresholds, and 1-minute..365-day limits.
- [x] Repeat validation during deserialization and reject unknown nested fields.
- [x] Prove distinct kinds/expiry precisions cannot coerce and `Debug` is private.

**Focused validator:**

```powershell
cargo +1.97.0 test -p tokenmaster-domain --test benefit_contract --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-domain --all-targets --locked
```

**Checkpoint commit:** `feat(domain): add provider benefit contracts`

---

### Task 2: Add the pure reconciliation and reminder core

**Files:**

- Create: `crates/benefits/Cargo.toml`
- Create: `crates/benefits/src/lib.rs`
- Create: `crates/benefits/src/identity.rs`
- Create: `crates/benefits/src/reconcile.rs`
- Create: `crates/benefits/src/reminder.rs`
- Create: `crates/benefits/tests/reconciliation_contract.rs`
- Create: `crates/benefits/tests/reminder_contract.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

**RED/GREEN:**

- [x] Add domain-separated architecture-independent scope/change/delivery identities.
- [x] Reconcile awarded/changed/unchanged/missing/reappeared/terminal lots without
  merging different identities or inventing terminal outcomes.
- [x] Return a bounded deterministic plan with exact next revisions/sequences.
- [x] Normalize recommended, subset, custom-only, and empty profiles.
- [x] Compute conservative due times, dedupe keys, and one most-urgent overdue
  delivery per lot.
- [x] Prove pure-core determinism, redaction, bounds, overflow handling, and no I/O
  dependency surface.

**Focused validator:**

```powershell
cargo +1.97.0 test -p tokenmaster-benefits --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-benefits --all-targets --locked
```

**Checkpoint commit:** `feat(benefits): add pure inventory reconciliation`

---

### Task 3: Normalize Codex reset-credit inventory

**Files:**

- Modify: `crates/codex/src/quota/mod.rs`
- Modify: `crates/codex/src/quota/normalize.rs`
- Modify: `crates/codex/src/lib.rs`
- Modify: `crates/codex/tests/quota_normalization_contract.rs`
- Modify: `crates/codex/tests/quota_transport_contract.rs`
- Modify: `crates/codex/tests/quota_transport_resource_contract.rs`

**RED/GREEN:**

- [ ] Expose one separate benefit observation in `CodexQuotaSnapshot`.
- [ ] Hash detailed raw IDs with account-separated framing before constructing lots.
- [ ] Emit one stable aggregate unknown-expiry lot for unexplained available count.
- [ ] Preserve different detailed IDs/expirations and map statuses conservatively.
- [ ] Reject duplicate IDs, count incoherence, invalid/overflow times, and excessive
  detail without leaking values.
- [ ] Prove raw ID/title/description/email cannot escape values, errors, `Debug`,
  serialized fixtures, or retained process output.

**Focused validator:**

```powershell
cargo +1.97.0 test -p tokenmaster-codex --test quota_normalization_contract --locked
cargo +1.97.0 test -p tokenmaster-codex --test quota_transport_contract --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-codex --all-targets --locked
```

**Checkpoint commit:** `feat(codex): normalize reset-credit inventory`

---

### Task 4: Add strict schema-v11 benefit storage

**Files:**

- Create: `crates/store/src/usage/benefit_schema.rs`
- Create: `crates/store/src/usage/benefit_types.rs`
- Create: `crates/store/src/usage/benefit_write.rs`
- Create: `crates/store/src/usage/benefit_maintenance.rs`
- Modify: `crates/store/src/usage/mod.rs`
- Modify: `crates/store/src/usage/schema.rs`
- Modify: `crates/store/src/usage/migration.rs`
- Modify: `crates/store/src/lib.rs`
- Modify: `crates/store/Cargo.toml`
- Create: `crates/store/tests/benefit_schema_contract.rs`
- Create: `crates/store/tests/benefit_write_contract.rs`
- Create: `crates/store/tests/benefit_retention_contract.rs`

**RED/GREEN:**

- [ ] Add strict state/scope/current/change/profile/threshold/due/delivery tables,
  exact contracts, indexes, and immutability triggers.
- [ ] Migrate exact v10 transactionally to v11 and validate fresh/v11 archives.
- [ ] Reconcile one scope observation atomically through the pure core.
- [ ] Keep duplicate polls history-neutral while refreshing bounded freshness.
- [ ] Rebuild due rows transactionally for changed lot/profile revisions.
- [ ] Enforce current/change/due/delivery bounds with protected current/ambiguous
  evidence and keyset maintenance.
- [ ] Inject faults at schema/current/history/due/revision boundaries and prove exact
  rollback plus unchanged usage/price/quota facts.

**Focused validator:**

```powershell
cargo +1.97.0 test -p tokenmaster-store --test benefit_schema_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test benefit_write_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test benefit_retention_contract --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-store --all-targets --locked
```

**Checkpoint commit:** `feat(store): add benefit inventory projection`

---

### Task 5: Add immutable benefit query snapshots

**Files:**

- Create: `crates/store/src/usage/query/benefit.rs`
- Modify: `crates/store/src/usage/query.rs`
- Modify: `crates/store/src/usage/mod.rs`
- Modify: `crates/store/src/lib.rs`
- Create: `crates/query/src/benefit.rs`
- Modify: `crates/query/src/service.rs`
- Modify: `crates/query/src/lib.rs`
- Create: `crates/query/tests/benefit_query_contract.rs`
- Create: `crates/query/tests/benefit_scale_contract.rs`

**RED/GREEN:**

- [ ] Add bounded current and keyset history store captures under one transaction.
- [ ] Sort current lots by conservative expiry, unknown expiry, kind, and opaque ID.
- [ ] Expose explicit absent/stale/partial/unknown facts and active inherited/override
  profile metadata.
- [ ] Expose nearest expiry/due and truthful `in_app_only` coverage.
- [ ] Bind continuation to exact scope and benefit revision with redacted `Debug`.
- [ ] Prove 64 lots, 2,048 changes, 256-row paging, restart, corruption rejection,
  generation neutrality, no usage scan, and resource return.

**Focused validator:**

```powershell
cargo +1.97.0 test -p tokenmaster-query --test benefit_query_contract --locked
cargo +1.97.0 test -p tokenmaster-query --test benefit_scale_contract --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-query --all-targets --locked
```

**Checkpoint commit:** `feat(query): expose immutable benefit snapshots`

---

### Task 6: Publish benefits through the Codex quota runtime

**Files:**

- Modify: `crates/runtime/src/quota/execution.rs`
- Modify: `crates/runtime/src/quota/health.rs`
- Modify: `crates/runtime/tests/quota_runtime_contract.rs`
- Modify: `crates/runtime/tests/quota_runtime_resource_contract.rs`

**RED/GREEN:**

- [ ] One source poll precedes one writer-lease attempt and one store open.
- [ ] Publish quota and benefit facts through separate exact transactions while the
  same non-interleaving guard is held.
- [ ] Report separate quota/benefit processed/changed/failure health.
- [ ] Prove quota success plus benefit failure, benefit success plus quota duplicate,
  cancellation, contention, retry, and restart behavior truthfully.
- [ ] Preserve usage-runtime isolation and bounded child/thread/handle/memory return.

**Focused validator:**

```powershell
cargo +1.97.0 test -p tokenmaster-runtime quota::execution --locked
cargo +1.97.0 test -p tokenmaster-runtime --test quota_runtime_contract --locked
cargo +1.97.0 test -p tokenmaster-runtime --test quota_runtime_resource_contract --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-runtime --all-targets --locked
```

**Checkpoint commit:** `feat(runtime): publish Codex benefit inventory`

---

### Task 7: Add one-timer durable reminder runtime

**Files:**

- Create: `crates/runtime/src/reminder/mod.rs`
- Create: `crates/runtime/src/reminder/execution.rs`
- Create: `crates/runtime/src/reminder/health.rs`
- Create: `crates/runtime/src/reminder/runtime.rs`
- Modify: `crates/runtime/src/lib.rs`
- Create: `crates/runtime/tests/reminder_runtime_contract.rs`
- Create: `crates/runtime/tests/reminder_runtime_resource_contract.rs`

**RED/GREEN:**

- [ ] Startup submits one recovery queue pass and waits only for nearest durable due.
- [ ] Process at most 256 rows and emit at most one urgent in-app delivery per lot.
- [ ] Record delivery before public notification publication and never duplicate a
  key across restart or clock changes.
- [ ] Profile/inventory changes, resume, hibernation, and wall-clock discontinuity
  coalesce to one reconciliation.
- [ ] Pause/shutdown join all task-owned state; reminder fault leaves usage/quota
  runtimes unchanged.
- [ ] Prove constant threads/timers, bounded memory/handles/USER/GDI, and no per-lot
  retained callbacks.

**Focused validator:**

```powershell
cargo +1.97.0 test -p tokenmaster-runtime --test reminder_runtime_contract --locked
cargo +1.97.0 test -p tokenmaster-runtime --test reminder_runtime_resource_contract --locked
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy -p tokenmaster-runtime --all-targets --locked
```

**Checkpoint commit:** `feat(runtime): add durable benefit reminders`

---

### Task 8: Close authority, documentation, and release gates

**Files:**

- Create: `scripts/audit-benefit-inventory.ps1`
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
- Modify: `docs/AUDIT_AND_MASTER_PLAN.md`
- Modify: `docs/FEATURE_PARITY.md`
- Modify this design and plan with exact verified evidence.

**Actions:**

- [ ] Audit dependency trees, production sources, SQLite strings, fixtures, and
  release libraries for raw IDs, provider payloads, browser/network/shell/credential
  authority, activation claims, and foreign runtimes.
- [ ] Record schema-v11 migration, identity/privacy, reconciliation, reminder
  coverage, failure isolation, retention, and resource evidence.
- [ ] Advance TM-DATA-009 only through inventory/reminder foundation; keep activation,
  OS notification scheduling, UI, CLI/MCP, and plugins incomplete.
- [ ] Inspect the complete diff and repository language/dependency surface.

**Baseline:**

```powershell
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS = '-Dwarnings'
cargo +1.97.0 clippy --workspace --all-targets --locked
cargo +1.97.0 test --workspace --locked
pwsh -NoProfile -File scripts\audit-benefit-inventory.ps1 -RepositoryRoot (Get-Location).Path
```

**Final checkpoint commit:** `docs(benefits): close inventory reminder contour`

## Stop conditions

Stop and report the exact blocker instead of weakening the contract if:

- the installed official Codex response cannot distinguish a trustworthy total from
  detailed rows;
- raw provider IDs or account data cannot be eliminated before the domain boundary;
- schema v10 cannot migrate transactionally without changing existing facts;
- duplicate polls or restart can duplicate history or reminder delivery;
- any read-only connector/plugin/CLI/MCP/LLM path acquires mutation authority;
- a focused resource test leaves a task-owned child, thread, handle, timer, or
  monotonic memory/SQLite growth;
- clean-root, strict Clippy, workspace tests, or authority audit fail for an in-scope
  reason that cannot be corrected without changing the approved architecture.
