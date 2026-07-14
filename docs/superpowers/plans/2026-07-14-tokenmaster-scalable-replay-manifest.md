# TokenMaster Scalable Replay Manifest Implementation Plan

**Status:** Complete. Implemented, verified, documented, committed, and pushed on
2026-07-14.

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> `superpowers:subagent-driven-development` (recommended) or
> `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox
> (`- [ ]`) syntax for tracking. The operator has deferred agents, so execute inline
> unless that instruction changes.

**Goal:** Remove the invalid 256-JSONL replay limit without history-sized application
memory by migrating to schema v3, creating all-source staging in SQLite, and validating
large manifests in bounded keyset pages.

**Architecture:** Preserve the explicit 256-key `ReplayManifest` as a bounded
test/repair API, but add a product `begin_replay_revision_all_sources()` transaction
that snapshots every registered source using set-based SQL. Widen persisted/runtime
source counts to checked `u64`, migrate exact v2 with SQLite's safe
create-new/copy/drop/rename procedure, and move full manifest validation into a focused
module that retains at most 256 source states.

**Tech Stack:** Rust 1.97.0, rusqlite 0.40.1 with bundled SQLite 3.51.3, strict SQLite
schema v3, PowerShell, existing TokenMaster store/accounting/domain crates.

## Global Constraints

- Work only on `cx/tokenmaster-product-architecture`; do not touch `main`.
- Test first for every behavior change; observe the expected RED result before code.
- Preserve all P0-D transaction, replay, rollback, legacy, read, and privacy behavior.
- Never allocate a collection from `expected_source_count`.
- Explicit manifest inputs remain capped at 256; product all-source begin is disk-backed.
- Manifest validation pages contain at most 256 fixed-size source states.
- SQLite source counts must be in `1..=i64::MAX`; Rust exposes checked `u64`.
- Do not use `PRAGMA writable_schema` or rename-old-first migration.
- Restore `PRAGMA foreign_keys=ON` after every successful or failed v2 migration attempt.
- Do not store or expose paths, raw JSON, prompts, responses, reasoning, commands,
  output, file contents, raw tails, or credentials.
- Do not add dependencies, threads, watchers, scan scheduling, source deletion, UI,
  CLI, MCP, packaging, or release behavior.
- Run the focused test first, then `cargo +1.97.0 test -p tokenmaster-store --locked`.
- Preserve unrelated work and never revert user changes.

---

### Task 1: Define exact schema v3 and migrate exact v2 safely

**Files:**

- Modify: `crates/store/src/usage/schema.rs`
- Modify: `crates/store/src/usage/migration.rs`
- Modify: `crates/store/tests/usage_schema_contract.rs`

**Interfaces:**

- Consumes: exact v1/v2 schema validation, `UsageStore::open`, strict table/index/
  trigger contracts, immutable legacy snapshot validation.
- Produces: `USAGE_SCHEMA_VERSION = 3`, exact v2 validator, exact v3 validator, direct
  v1-to-v3 migration, fault-tested v2-to-v3 migration, and restored foreign-key policy.

- [x] **Step 1: Write the failing public schema assertions**

In `usage_schema_contract.rs`, change the fresh/current expectations to schema 3 and
replace the old upper-bound assertion with exact v3 checks:

```rust
assert_eq!(USAGE_SCHEMA_VERSION, 3);
assert_eq!(user_version(&path), 3);
let revision_sql = table_sql(&path, "usage_replay_revision");
assert!(revision_sql.contains(
    "expected_source_count INTEGER NOT NULL CHECK(expected_source_count >= 1)"
));
assert!(!revision_sql.contains("expected_source_count BETWEEN 1 AND 256"));
```

Retain every existing table, index, trigger, strict-mode, privacy, malformed-schema,
legacy-copy, and immutability assertion.

- [x] **Step 2: Run the public contract and record RED**

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-store --test usage_schema_contract --locked
```

Expected: failures show actual schema/user version `2` and the old
`BETWEEN 1 AND 256` constraint.

- [x] **Step 3: Add internal exact-v2 migration and fault tests**

In `migration.rs` add a `#[cfg(test)]` module that uses private schema constants to
create exact v2 in memory. Seed:

- one current replay revision with all child-table rows;
- one separate staging-revision fixture in a second database;
- one immutable legacy snapshot/event;
- current and staging generations plus chunk/observation/selection/work rows.

Add these tests with no direct `unwrap`/`expect`:

```rust
#[test]
fn exact_v2_rebuilds_only_revision_table_and_preserves_all_rows() -> TestResult

#[test]
fn every_v2_migration_fault_rolls_back_and_restores_foreign_keys() -> TestResult

#[test]
fn malformed_v2_is_rejected_before_foreign_keys_are_disabled() -> TestResult
```

The success test compares ordered row values before/after, asserts no table or foreign
key references `usage_replay_revision_v3`, verifies `user_version = 3`, confirms the
legacy triggers still reject insert/update/delete, and runs `foreign_key_check`.

The fault test loops over:

```rust
MigrationFault::AfterCreateRevision
MigrationFault::AfterCopyRevision
MigrationFault::AfterDropRevision
```

After every injected failure require `user_version = 2`, exact-v2 validation succeeds,
all seed rows match, and `PRAGMA foreign_keys = 1`.

- [x] **Step 4: Run the internal migration tests and record RED**

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-store usage::migration::tests --locked
```

Expected: compile failures because v3 constants, migration fault seam, and v2 branch do
not exist.

- [x] **Step 5: Split replay schema strings without changing v2 SQL**

In `schema.rs` retain exact historical v2 SQL by splitting the current
`V2_REPLAY_SCHEMA` at the revision table:

```rust
pub const USAGE_SCHEMA_VERSION: i64 = 3;
pub(super) const V2_SCHEMA_VERSION: i64 = 2;

pub(super) const V2_REPLAY_REVISION_SCHEMA: &str = r#"
CREATE TABLE usage_replay_revision (
  revision_id INTEGER PRIMARY KEY CHECK(revision_id >= 0),
  status TEXT NOT NULL CHECK(status IN ('staging','current')),
  canonicalizer_version INTEGER NOT NULL CHECK(canonicalizer_version BETWEEN 1 AND 65535),
  fingerprint_version INTEGER NOT NULL CHECK(fingerprint_version BETWEEN 1 AND 65535),
  replay_signature_version INTEGER NOT NULL CHECK(replay_signature_version BETWEEN 1 AND 65535),
  expected_source_count INTEGER NOT NULL CHECK(expected_source_count BETWEEN 1 AND 256),
  evidence_epoch INTEGER NOT NULL CHECK(evidence_epoch >= 0),
  sealed INTEGER NOT NULL CHECK(sealed IN (0,1)),
  promoted INTEGER NOT NULL CHECK(promoted IN (0,1)),
  CHECK((status = 'staging' AND promoted = 0) OR
        (status = 'current' AND sealed = 1 AND promoted = 1))
) STRICT;
"#;

pub(super) const V3_REPLAY_REVISION_SCHEMA: &str = r#"
CREATE TABLE usage_replay_revision (
  revision_id INTEGER PRIMARY KEY CHECK(revision_id >= 0),
  status TEXT NOT NULL CHECK(status IN ('staging','current')),
  canonicalizer_version INTEGER NOT NULL CHECK(canonicalizer_version BETWEEN 1 AND 65535),
  fingerprint_version INTEGER NOT NULL CHECK(fingerprint_version BETWEEN 1 AND 65535),
  replay_signature_version INTEGER NOT NULL CHECK(replay_signature_version BETWEEN 1 AND 65535),
  expected_source_count INTEGER NOT NULL CHECK(expected_source_count >= 1),
  evidence_epoch INTEGER NOT NULL CHECK(evidence_epoch >= 0),
  sealed INTEGER NOT NULL CHECK(sealed IN (0,1)),
  promoted INTEGER NOT NULL CHECK(promoted IN (0,1)),
  CHECK((status = 'staging' AND promoted = 0) OR
        (status = 'current' AND sealed = 1 AND promoted = 1))
) STRICT;
"#;
```

Move the existing statements from `CREATE TABLE usage_legacy_snapshot` through the
last `usage_legacy_event` index, without editing their text, into
`REPLAY_AUX_SCHEMA`. Move the existing two `usage_replay_revision` partial indexes and
the complete statements for `usage_replay_source`, `usage_replay_session`,
`usage_replay_observation`, its four indexes, `usage_replay_selection`, and
`usage_replay_work`, without editing their text, into `REPLAY_CHILD_SCHEMA`. Update
schema-source arrays so v2 validation searches `V2_REPLAY_REVISION_SCHEMA`, while v3
searches `V3_REPLAY_REVISION_SCHEMA`; every other table uses the two common exact
fragments. The integration schema contract verifies that this mechanical split changes
no table/index/trigger other than the intended count constraint.

- [x] **Step 6: Implement v2-to-v3 migration with guaranteed policy restoration**

Refactor `migrate_schema` into version-specific paths. Versions 0, 1, and 3 may use the
existing immediate-transaction helper. Version 2 must validate before mutation, then
disable foreign keys outside the migration transaction:

```rust
fn migrate_v2_with_fault(
    connection: &mut Connection,
    fault: MigrationFault,
) -> Result<(), StoreError> {
    validate_v2(connection)?;
    connection.pragma_update(None, "foreign_keys", "OFF")?;
    if pragma_i64(connection, "PRAGMA foreign_keys")? != 0 {
        return Err(StoreError::new(StoreErrorCode::PolicyMismatch));
    }

    let migration: Result<(), StoreError> = (|| {
        let transaction =
            connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        migrate_v2_revision_table(&transaction, fault)?;
        transaction.commit()?;
        Ok(())
    })();

    let restored = connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(StoreError::from)
        .and_then(|()| {
            if pragma_i64(connection, "PRAGMA foreign_keys")? == 1 {
                Ok(())
            } else {
                Err(StoreError::new(StoreErrorCode::PolicyMismatch))
            }
        });

    match (migration, restored) {
        (Ok(()), Ok(())) => validate_v3(connection),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(_migration_error), Err(_restoration_error)) => {
            Err(StoreError::new(StoreErrorCode::PolicyMismatch))
        }
    }
}
```

Ensure the transaction is dropped before restoration on every error. If both migration
and restoration fail, return a stable store error and do not expose SQLite text.
Define `MigrationFault` with an ordinary `None` variant and test-only named fault
variants; production `migrate_v2` calls `migrate_v2_with_fault(connection,
MigrationFault::None)` so fault injection cannot change the public store interface.

Use the exact safe SQL order:

```sql
CREATE TABLE usage_replay_revision_v3 (
  revision_id INTEGER PRIMARY KEY CHECK(revision_id >= 0),
  status TEXT NOT NULL CHECK(status IN ('staging','current')),
  canonicalizer_version INTEGER NOT NULL CHECK(canonicalizer_version BETWEEN 1 AND 65535),
  fingerprint_version INTEGER NOT NULL CHECK(fingerprint_version BETWEEN 1 AND 65535),
  replay_signature_version INTEGER NOT NULL CHECK(replay_signature_version BETWEEN 1 AND 65535),
  expected_source_count INTEGER NOT NULL CHECK(expected_source_count >= 1),
  evidence_epoch INTEGER NOT NULL CHECK(evidence_epoch >= 0),
  sealed INTEGER NOT NULL CHECK(sealed IN (0,1)),
  promoted INTEGER NOT NULL CHECK(promoted IN (0,1)),
  CHECK((status = 'staging' AND promoted = 0) OR
        (status = 'current' AND sealed = 1 AND promoted = 1))
) STRICT;
INSERT INTO usage_replay_revision_v3(
  revision_id, status, canonicalizer_version, fingerprint_version,
  replay_signature_version, expected_source_count, evidence_epoch, sealed, promoted
)
SELECT
  revision_id, status, canonicalizer_version, fingerprint_version,
  replay_signature_version, expected_source_count, evidence_epoch, sealed, promoted
FROM usage_replay_revision;
DROP TABLE usage_replay_revision;
ALTER TABLE usage_replay_revision_v3 RENAME TO usage_replay_revision;
CREATE UNIQUE INDEX usage_replay_revision_one_current
  ON usage_replay_revision(status) WHERE status = 'current';
CREATE UNIQUE INDEX usage_replay_revision_one_staging
  ON usage_replay_revision(status) WHERE status = 'staging';
```

Compare old/new revision counts before drop, inject each fault at the named boundary,
run `foreign_key_check`, set `user_version = 3`, and validate exact v3 before commit.
Never enable `legacy_alter_table` or `writable_schema`.

Fresh creation and v1 migration execute common replay auxiliary SQL, the v3 revision
SQL, common child SQL, legacy copy when applicable, and immutability triggers.

- [x] **Step 7: Verify schema/migration GREEN**

Run:

```powershell
cargo +1.97.0 test -p tokenmaster-store --test usage_schema_contract --locked
cargo +1.97.0 test -p tokenmaster-store usage::migration::tests --locked
cargo +1.97.0 test -p tokenmaster-store --locked
```

Expected: exact fresh/v1/v2/v3, rollback, immutability, policy, and prior store tests
pass. The one-million-row test may remain explicitly ignored.

- [x] **Step 8: Commit schema v3**

```powershell
git add -- crates/store/src/usage/schema.rs crates/store/src/usage/migration.rs crates/store/tests/usage_schema_contract.rs
git diff --cached --check
git commit -m "feat(store): migrate replay manifest to schema v3"
```

---

### Task 2: Widen source counts and add disk-backed all-source begin

**Files:**

- Create: `crates/store/src/usage/replay_manifest.rs`
- Modify: `crates/store/src/usage/mod.rs`
- Modify: `crates/store/src/usage/types.rs`
- Modify: `crates/store/src/usage/replay.rs`
- Modify: `crates/store/tests/replay_archive_contract.rs`

**Interfaces:**

- Consumes: schema v3, `UsageStore`, current source/generation rows, compiled accounting
  versions, existing explicit `begin_replay_revision(&ReplayManifest)`.
- Produces: `UsageStore::begin_replay_revision_all_sources()`, `u64` expected source
  counts, atomic set-based staging over every registered source.

- [x] **Step 1: Write failing >256 all-source begin tests**

Add path-private helpers in `replay_archive_contract.rs`:

```rust
fn source_key_for_index(index: u32) -> SourceKey {
    let mut bytes = [0_u8; 32];
    bytes[..4].copy_from_slice(&index.to_be_bytes());
    SourceKey::from_bytes(bytes)
}

fn digest_for_index(index: u32, tag: u8) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    bytes[..4].copy_from_slice(&index.to_be_bytes());
    bytes[4] = tag;
    bytes
}

fn registration_for_index(index: u32) -> SourceRegistration {
    SourceRegistration::new(SourceRegistrationParts {
        source_key: source_key_for_index(index),
        provider_id: "codex".into(),
        profile_id: "large-fixture".into(),
        source_id: format!("fixture-{index}").into_boxed_str(),
        source_kind: SourceKind::Active,
        logical_identity: digest_for_index(index, 1),
        physical_identity: Some(digest_for_index(index, 2)),
        initial_checkpoint: StoredCheckpoint::new(StoredCheckpointParts {
            parser_schema_version: 1,
            physical_identity: Some(digest_for_index(index, 2)),
            logical_identity: digest_for_index(index, 1),
            committed_offset: 0,
            scan_offset: 0,
            observed_file_length: 0,
            modified_time_ns: None,
            anchor_start: 0,
            anchor_len: 0,
            anchor_sha256: digest_for_index(index, 3),
            resume: Box::default(),
            discarding_oversized_line: false,
            incomplete_tail: false,
            verification: StoredVerification::Incremental,
        })
        .expect("valid large-fixture checkpoint"),
    })
    .expect("valid large-fixture registration")
}
```

Add:

```rust
#[test]
fn all_source_begin_stages_three_hundred_sources_without_a_manifest_vector()

#[test]
fn all_source_begin_is_atomic_on_empty_missing_current_and_generation_overflow()
```

The success test registers 300 sources, records current counts/pages/pointers, calls
`begin_replay_revision_all_sources`, and requires:

- `expected_source_count() == 300_u64`;
- exactly 300 staging generation/replay-source rows after reopen;
- zero changed current pointers or canonical events;
- a second staging begin returns `ArchiveModeMismatch` without writes.

The failure test covers zero registered sources, a raw synthetic fixture whose valid
`usage_source.current_generation` is `NULL`, and a valid raw fixture whose current
generation is `i64::MAX`. To create the overflow fixture without violating foreign
keys, clear the source pointer, remove generation zero, insert a current generation at
`i64::MAX`, then repoint the source. Each case must leave zero replay/staging rows.

- [x] **Step 2: Run the focused test and record RED**

```powershell
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract all_source_begin --locked
```

Expected: compile failure because `begin_replay_revision_all_sources` does not exist.

- [x] **Step 3: Widen count types without widening explicit manifest input**

In `types.rs` change only revision count storage/access:

```rust
pub struct ReplayRevisionSnapshot {
    pub(super) id: ReplayRevisionId,
    pub(super) epoch: ReplayEpoch,
    pub(super) status: ReplayRevisionStatus,
    pub(super) versions: AccountingVersions,
    pub(super) expected_source_count: u64,
    pub(super) sealed: bool,
    pub(super) promoted: bool,
}

impl ReplayRevisionSnapshot {
    #[must_use]
    pub const fn expected_source_count(self) -> u64 {
        self.expected_source_count
    }
}
```

Keep `MAX_REPLAY_SOURCES = 256` and `ReplayManifest::new(Box<[SourceKey]>)` unchanged.
In `replay.rs`, change `StoredRevision.expected_source_count` and every helper parameter
to `u64`. Read SQLite counts with checked `u64::try_from`, reject zero and values above
`i64::MAX`, and compare SQL mutation counts through checked `u64::try_from(usize)`.
Never convert the stored count to `usize` for capacity/allocation.

- [x] **Step 4: Implement all-source begin in a focused module**

Register `mod replay_manifest;` in `usage/mod.rs`. In the new file define the empty
SHA-256 digest and implement the public method
`UsageStore::begin_replay_revision_all_sources(&mut self) ->
Result<ReplayRevisionSnapshot, StoreError>`.

The method must start `TransactionBehavior::Immediate`, reject nonzero counts
for either staging revisions or staging generations, read the checked registered count,
reject zero, require exactly one `status='current'` generation joined through every
`usage_source.current_generation`, reject any per-source maximum generation equal to
`i64::MAX`, derive the next revision ID/compiled versions/epoch zero, insert the
revision header, run the two set statements below, compare revision/source/generation
row counts, validate `foreign_key_check`, commit, and return the complete snapshot.

Use set-based statements. The generation insert must select all current sources and
derive the next generation without an application loop:

```sql
INSERT INTO usage_generation(
  file_key, generation, status, parser_schema_version, physical_identity,
  logical_identity, committed_offset, scan_offset, observed_file_length,
  modified_time_ns, anchor_start, anchor_len, anchor_sha256, resume_payload,
  discarding_oversized_line, incomplete_tail, verification_level
)
SELECT
  source.file_key,
  (SELECT max(previous.generation) + 1
   FROM usage_generation AS previous
   WHERE previous.file_key = source.file_key),
  'staging', current.parser_schema_version, current.physical_identity,
  current.logical_identity, 0, 0, 0, NULL, 0, 0, ?1, zeroblob(0),
  0, 0, 'incremental'
FROM usage_source AS source
JOIN usage_generation AS current
  ON current.file_key = source.file_key
 AND current.generation = source.current_generation
WHERE current.status = 'current'
ORDER BY source.file_key;
```

Preflight generation overflow with an aggregate query so SQLite never promotes an
overflowed arithmetic value. Insert replay-source rows by joining the new
`status='staging'` generations. Verify each inserted row count equals the stored
`u64` source count and validate foreign keys before commit.

The existing explicit begin remains behavior-compatible and uses the widened count.

- [x] **Step 5: Verify all-source begin GREEN and regressions**

```powershell
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract all_source_begin --locked
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract --locked
cargo +1.97.0 test -p tokenmaster-store --locked
```

Expected: 300-source begin is atomic/invisible and every prior P0-D test passes.

- [x] **Step 6: Commit disk-backed begin**

```powershell
git add -- crates/store/src/usage/replay_manifest.rs crates/store/src/usage/mod.rs crates/store/src/usage/types.rs crates/store/src/usage/replay.rs crates/store/tests/replay_archive_contract.rs
git diff --cached --check
git commit -m "feat(store): stage all registered replay sources"
```

---

### Task 3: Keyset-page complete-manifest validation and large lifecycle

**Files:**

- Modify: `crates/store/src/usage/replay_manifest.rs`
- Modify: `crates/store/src/usage/replay.rs`
- Modify: `crates/store/tests/replay_archive_contract.rs`

**Interfaces:**

- Consumes: schema v3 count, all-source begin, staging append, continuation, seal,
  promotion, discard.
- Produces: aggregate closed-source check for continuation, full 256-row keyset
  validation for seal/promotion, successful >256-source lifecycle.

- [x] **Step 1: Write failing paged lifecycle tests**

Add:

```rust
#[test]
fn three_hundred_sources_complete_seal_promote_and_reopen_in_pages()

#[test]
fn source_registered_after_all_source_begin_blocks_seal_without_mutation()
```

For the first test, register 300 empty sources, begin all-source replay, and apply one
empty replay append per source. Each append uses its deterministic fresh generation 1,
zero offsets/chunks/events, and a `StoredCheckpoint` marked `FullPrefix`. Carry the
returned epoch into the next append. Then seal/promote, reopen, and require:

- `ReplayVerified`, no staging revision/generation;
- exact revision source count 300;
- every current pointer is generation 1 and `full_prefix`;
- canonical page stays empty and foreign keys are clean.

The second test begins over 300, registers source 301 afterward, and proves seal returns
`IncompleteManifest`, epoch/page/pointers/staging counts remain unchanged, and exact
discard still restores the prior archive.

- [x] **Step 2: Run the lifecycle tests and record RED**

```powershell
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract three_hundred --locked
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract source_registered_after --locked
```

Expected: current complete-manifest validation stops at `LIMIT 257`/narrow count or
rejects the large lifecycle.

- [x] **Step 3: Add cheap closed-source aggregate for continuation**

In `replay_manifest.rs` implement:

```rust
pub(super) fn replay_manifest_sources_closed(
    transaction: &Transaction<'_>,
    revision_id: ReplayRevisionId,
    expected_source_count: u64,
) -> Result<bool, StoreError>
```

One aggregate query must return registered-source count, replay-source count,
replay-source rows whose state is `complete`, and correctly owned staging-generation
count. Convert every count through checked `u64`; return true only when all four equal
`expected_source_count`.

Change `continue_replay` to call this cheap aggregate when deciding whether
missing-parent work is actionable. Do not full-scan chunk rows for every continuation
item. Final seal/promotion still perform exact validation.

- [x] **Step 4: Implement 256-row keyset full validation**

Move `ManifestSourceState`, `replay_manifest_is_complete`, `source_chunks_cover`, and
`validate_complete_manifest` from the 3,000-line `replay.rs` into
`replay_manifest.rs`. Export only the two validators as `pub(super)`.

Use:

```rust
const MANIFEST_VALIDATION_PAGE_SIZE: usize = 256;
```

The validator initializes `cursor: Option<[u8; 32]> = None` and `visited = 0_u64`.
Each loop loads at most 256 rows strictly after the cursor. An empty page terminates.
For every row, require replay state `complete`, generation status `staging`, checkpoint
verification `full_prefix`, and equal committed/scan/observed offsets; then call
`source_chunks_cover` with its file key, generation, and committed offset. Add the page
length to `visited` through checked `u64::try_from` and `checked_add`, returning
`InvalidStoredValue` on conversion failure and `CapacityExceeded` on addition overflow.
Set the cursor to the last file key. A short page terminates; a full page performs the
next query. Return `Ok(visited == expected_source_count)` only after the loop.

The page query uses `file_key > ?cursor`, `ORDER BY file_key`, `LIMIT 256`; when cursor
is absent use a separate first-page query or a nullable predicate that preserves index
order. The `Vec` capacity is exactly 256, never the expected count. Validate aggregate
registered/manifest/staging counts before paging and reject any count mismatch.

Keep exact chunk rules: zero offset requires zero chunks; nonzero coverage requires
contiguous indices from zero, full 1 MiB chunks except the final exact length, and total
covered bytes equal committed offset.

- [x] **Step 5: Make promotion count checks 64-bit safe**

Change all promotion comparisons from `usize::from(expected_source_count)` or
`i64::from(expected_source_count)` to checked conversions:

```rust
fn mutation_count(value: usize) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| StoreError::new(StoreErrorCode::InvalidStoredValue))
}
```

Use checked stored-count helpers for SQL aggregates. Do not cast with `as`.

- [x] **Step 6: Verify paged lifecycle GREEN**

```powershell
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract three_hundred --locked
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract source_registered_after --locked
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract --locked
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy -p tokenmaster-store --all-targets --locked
Remove-Item Env:RUSTFLAGS
cargo +1.97.0 test -p tokenmaster-store --locked
```

Expected: large lifecycle crosses at least two validation pages, all existing replay
and rollback tests pass, Clippy is warning-free, and the normal scale test remains
explicitly ignored unless run separately.

- [x] **Step 7: Commit paged validation**

```powershell
git add -- crates/store/src/usage/replay_manifest.rs crates/store/src/usage/replay.rs crates/store/tests/replay_archive_contract.rs
git diff --cached --check
git commit -m "feat(store): validate replay manifests in pages"
```

---

### Task 4: Close P0-D.1 truth and full quality gates

**Files:**

- Modify: `spec/DATA_CONTRACT.md`
- Modify: `spec/SECURITY.md`
- Modify: `spec/DECISIONS.md`
- Modify: `spec/TRACEABILITY.md`
- Modify: `docs/CURRENT_STATE.md`
- Modify: `docs/PROJECT_HISTORY.md`
- Modify: `docs/HANDOFF.md`
- Modify: `docs/ROADMAP.md`
- Modify: `docs/RECOVERY_PLAYBOOK.md`
- Modify: `docs/superpowers/specs/2026-07-14-tokenmaster-scalable-replay-manifest-design.md`
- Modify: `docs/superpowers/specs/2026-07-14-tokenmaster-p0-e-pipeline-proof-design.md`
- Modify: `docs/superpowers/plans/2026-07-14-tokenmaster-scalable-replay-manifest.md`

**Interfaces:**

- Consumes: completed schema v3 migration, all-source begin, paged seal/promotion,
  focused/full verification evidence.
- Produces: consistent durable project truth with P0-D.1 complete and P0-E unblocked.

- [x] **Step 1: Update contracts and history**

Record exactly:

- schema v3 and non-destructive exact v2 migration;
- product all-source begin is SQLite-owned and O(1) application manifest memory;
- explicit `ReplayManifest` remains capped at 256 and cannot seal a subset;
- source count is checked `u64`/SQLite signed integer and never allocation authority;
- full seal/promotion validation is keyset-paged at 256; continuation uses only the
  transactional closed-source aggregate and cannot promote tampered data;
- no missing-source reconciliation, engine, query, UI, automation, package, or M0
  acceptance is claimed;
- P0-E transactional pipeline proof is next, with fixtures beyond 256 files/events.

Add an ADR for disk-backed all-source manifests and exact schema v3 migration. Mark the
scalable-manifest design/plan complete only after the gates below pass.

- [x] **Step 2: Run focused and full verification from fresh commands**

```powershell
cargo +1.97.0 test -p tokenmaster-store --test usage_schema_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test replay_archive_contract --locked
cargo +1.97.0 test -p tokenmaster-store --test usage_ingest_contract --locked
cargo +1.97.0 test -p tokenmaster-accounting --test replay_classifier_contract --locked
pwsh -NoProfile -File scripts\audit-clean-root.ps1 -RepositoryRoot (Get-Location).Path
cargo +1.97.0 fmt --all -- --check
$env:RUSTFLAGS='-Dwarnings'; cargo +1.97.0 clippy --workspace --all-targets --locked
Remove-Item Env:RUSTFLAGS
cargo +1.97.0 test --workspace --locked
git diff --check
```

Expected: all commands pass; report the existing ignored one-million-row M0 test
without implying it ran.

- [x] **Step 3: Run privacy, schema, and scalability audits**

Verify changed Rust/schema contains no forbidden retained-content identifiers or
absolute user paths; diff contains no secret-like values; `rg` finds no product claim
that a complete replay manifest is limited to 256. The only remaining 256-source text
must clearly describe the explicit test/repair input or the historical superseded
design.

Confirm:

```powershell
git status --short --branch
git diff --stat
git diff --name-only
git diff --check
```

- [x] **Step 4: Perform an independent root review**

Review every diff against the scalable-manifest design and SQLite migration order.
Confirm no allocation uses `expected_source_count`, no full source list is collected,
foreign keys are restored on all tested exits, and no P0-E/P1 behavior leaked into
P0-D.1. Agents remain deferred unless the operator explicitly re-enables them.

- [x] **Step 5: Commit, push, and verify remote identity**

```powershell
git add -- spec/DATA_CONTRACT.md spec/SECURITY.md spec/DECISIONS.md spec/TRACEABILITY.md docs/CURRENT_STATE.md docs/PROJECT_HISTORY.md docs/HANDOFF.md docs/ROADMAP.md docs/RECOVERY_PLAYBOOK.md docs/superpowers/specs/2026-07-14-tokenmaster-scalable-replay-manifest-design.md docs/superpowers/specs/2026-07-14-tokenmaster-p0-e-pipeline-proof-design.md docs/superpowers/plans/2026-07-14-tokenmaster-scalable-replay-manifest.md
git diff --cached --check
git commit -m "docs: record scalable replay manifest"
git push origin cx/tokenmaster-product-architecture
git status --short --branch
git rev-parse HEAD
git rev-parse '@{upstream}'
```

Expected: worktree clean and local/remote full SHA values identical. Do not merge,
create a release, package, or claim M0/interactive evidence.

## Stop conditions

Stop and report exact evidence rather than weakening a contract if:

- exact populated v2 cannot migrate without changing child/legacy values;
- any failed migration leaves `foreign_keys=OFF`;
- safe create-new/copy/drop/rename cannot preserve child foreign-key references;
- all-source begin constructs a history-sized Rust collection;
- >256 sources cannot seal/promote with at most 256 retained validation states;
- a subset explicit manifest can become canonical while registered sources are omitted;
- any earlier P0-D rollback, privacy, restart, or legacy test regresses unexplained;
- schema v3 requires `writable_schema`, arbitrary SQL exposure, or a new dependency.
